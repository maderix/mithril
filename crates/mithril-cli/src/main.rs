//! `mithril` — CLI driver over the front/reassoc/net/codegen crates.
//!
//! Subcommands:
//! - `mithril run f.py [--threads N] [--gpu]` — full pipeline, execute.
//! - `mithril build f.py -o out`              — same, binary copied to `out`.
//! - `mithril net f.py`                       — print the reduced residual net.
//! - `mithril prove f.py`                     — write + check obligations.lean.
//!
//! Any `Diag` exits 1 with `line <n>: <msg>` on stderr. The compiled
//! `mithril_rt` rlib (and its `mithril_core` dep) is cached under
//! `target/mithril-cache/`, keyed by `rustc -V`.

use mithril_front::ast::Module;
use mithril_front::core::CoreModule;
use mithril_front::Diag;
use mithril_reassoc::FoldReport;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Fuel for the compile-time net reducer.
const REDUCE_FUEL: u64 = 1 << 20;

// ---------------------------------------------------------------- errors

enum CliErr {
    Diag(Diag),
    Other(String),
}

impl From<Diag> for CliErr {
    fn from(d: Diag) -> CliErr {
        CliErr::Diag(d)
    }
}
impl From<String> for CliErr {
    fn from(s: String) -> CliErr {
        CliErr::Other(s)
    }
}
impl From<&str> for CliErr {
    fn from(s: &str) -> CliErr {
        CliErr::Other(s.to_string())
    }
}
impl From<std::io::Error> for CliErr {
    fn from(e: std::io::Error) -> CliErr {
        CliErr::Other(e.to_string())
    }
}

// ------------------------------------------------------------ entry point

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(code) => std::process::exit(code),
        Err(CliErr::Diag(d)) => {
            eprintln!("line {}: {}", d.line, d.msg);
            std::process::exit(1);
        }
        Err(CliErr::Other(m)) => {
            eprintln!("error: {}", m);
            std::process::exit(1);
        }
    }
}

const USAGE: &str = "usage: mithril <run|build|net|prove> f.py [--threads N] [--gpu] [-o out]";

fn dispatch(args: &[String]) -> Result<i32, CliErr> {
    match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("build") => cmd_build(&args[1..]),
        Some("net") => cmd_net(&args[1..]),
        Some("prove") => cmd_prove(&args[1..]),
        Some(other) => Err(format!("unknown subcommand '{}'\n{}", other, USAGE).into()),
        None => Err(USAGE.into()),
    }
}

// ------------------------------------------------------------ option parse

struct Opts {
    file: PathBuf,
    threads: Option<usize>,
    gpu: bool,
    out: Option<PathBuf>,
}

fn parse_opts(args: &[String]) -> Result<Opts, CliErr> {
    let (mut file, mut threads, mut gpu, mut out) = (None, None, false, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--threads" => {
                i += 1;
                let v = args.get(i).ok_or("--threads needs a value")?;
                let n: usize = v.parse().map_err(|_| format!("--threads: not a count: '{}'", v))?;
                threads = Some(n.max(1));
            }
            "--gpu" => gpu = true,
            "-o" => {
                i += 1;
                out = Some(PathBuf::from(args.get(i).ok_or("-o needs a path")?));
            }
            s if s.starts_with('-') => return Err(format!("unknown option '{}'\n{}", s, USAGE).into()),
            s => {
                if file.is_some() {
                    return Err(format!("unexpected argument '{}'\n{}", s, USAGE).into());
                }
                file = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }
    Ok(Opts { file: file.ok_or("missing input file")?, threads, gpu, out })
}

// ------------------------------------------------------------ front stages

fn front(path: &Path) -> Result<(Module, Vec<FoldReport>), CliErr> {
    let src = fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let mut m = mithril_front::parse(&src)?;
    let reports = mithril_reassoc::analyze(&mut m);
    Ok((m, reports))
}

fn to_core(m: &Module) -> Result<CoreModule, CliErr> {
    // Desugar falls back to fns[0] when nothing is named `main`; the CLI
    // requires an explicit entry point.
    if !m.fns.iter().any(|f| f.name == "main") {
        return Err(Diag::new(1, "no main function").into());
    }
    Ok(mithril_front::desugar(m)?)
}

/// The front stages and the net specialization, with its reports.
fn specialized(o: &Opts) -> Result<(CoreModule, Vec<mithril_net::SpecReport>), CliErr> {
    let (m, _) = front(&o.file)?;
    Ok(mithril_net::specialize(&to_core(&m)?, REDUCE_FUEL))
}

// -------------------------------------------------------- paths and cache

fn workspace_root() -> PathBuf {
    // crates/mithril-cli -> crates -> workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn target_dir() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn make_temp_dir() -> Result<PathBuf, CliErr> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let d = env::temp_dir().join(format!(
        "mithril-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d)?;
    Ok(d)
}

fn run_tool(mut c: Command, what: &str) -> Result<(), CliErr> {
    let out = c.output().map_err(|e| format!("cannot run {}: {}", what, e))?;
    if !out.status.success() {
        return Err(format!("{} failed:\n{}", what, String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(())
}

fn rustc_rlib(out_dir: &Path, name: &str, src: &Path) -> Result<(), CliErr> {
    let mut c = Command::new("rustc");
    c.args(["--edition", "2021", "-O", "--crate-type", "rlib", "--crate-name", name])
        .arg(src)
        .arg("--out-dir")
        .arg(out_dir)
        .arg("-L")
        .arg(out_dir);
    let core = out_dir.join("libmithril_core.rlib");
    if name != "mithril_core" && core.exists() {
        c.arg("--extern").arg(format!("mithril_core={}", core.display()));
    }
    run_tool(c, &format!("rustc ({})", name))
}

/// Compile (or reuse) the cached `mithril_rt` rlib + its `mithril_core`
/// dep under `target/mithril-cache/rt-<hash of rustc -V + rt/core sources>/`.
fn rt_cache_dir() -> Result<PathBuf, CliErr> {
    let ver = Command::new("rustc")
        .arg("-V")
        .output()
        .map_err(|e| format!("cannot run rustc: {}", e))?;
    if !ver.status.success() {
        return Err("rustc -V failed".into());
    }
    // key: rustc version + the runtime/core sources, so edits to the
    // runtime never serve a stale rlib
    let crates_root = workspace_root().join("crates");
    let mut key: Vec<u8> = ver.stdout.clone();
    for c in ["mithril-core", "mithril-rt"] {
        let src = crates_root.join(c).join("src");
        if let Ok(rd) = fs::read_dir(&src) {
            let mut names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            names.sort();
            for f in names {
                if let Ok(bytes) = fs::read(&f) {
                    key.extend_from_slice(&bytes);
                }
            }
        }
    }
    let base = target_dir().join("mithril-cache");
    let dir = base.join(format!("rt-{:016x}", fnv64(&key)));
    if dir.join("libmithril_rt.rlib").exists() {
        return Ok(dir);
    }
    fs::create_dir_all(&base)?;
    let tmp = base.join(format!("tmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp)?;
    rustc_rlib(&tmp, "mithril_core", &crates_root.join("mithril-core/src/lib.rs"))?;
    rustc_rlib(&tmp, "mithril_rt", &crates_root.join("mithril-rt/src/lib.rs"))?;
    match fs::rename(&tmp, &dir) {
        Ok(()) => {}
        // A concurrent invocation won the race; use its cache.
        Err(_) if dir.join("libmithril_rt.rlib").exists() => {
            let _ = fs::remove_dir_all(&tmp);
        }
        Err(e) => return Err(format!("installing rt cache failed: {}", e).into()),
    }
    Ok(dir)
}

// ----------------------------------------------------------- compile step

/// Emit Rust for the specialized module and `rustc -O` it to `out_bin`.
fn compile_program(cm: &CoreModule, out_bin: &Path) -> Result<(), CliErr> {
    let src = mithril_codegen::emit_rust(cm);
    let cache = rt_cache_dir()?;
    let tmp = make_temp_dir()?;
    fs::write(tmp.join("main.rs"), src)?;
    let mut c = Command::new("rustc");
    c.args(["--edition", "2021", "-O"])
        .arg(tmp.join("main.rs"))
        .arg("-o")
        .arg(out_bin)
        .arg("-L")
        .arg(&cache)
        .arg("--extern")
        .arg(format!("mithril_rt={}", cache.join("libmithril_rt.rlib").display()));
    run_tool(c, "rustc (generated program)")?;
    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

// ------------------------------------------------------------ subcommands

fn cmd_run(args: &[String]) -> Result<i32, CliErr> {
    let o = parse_opts(args)?;
    let (sm, _) = specialized(&o)?;
    if o.gpu {
        return run_gpu(&sm);
    }
    let tmp = make_temp_dir()?;
    let bin = tmp.join("prog");
    compile_program(&sm, &bin)?;
    let mut c = Command::new(&bin);
    if let Some(t) = o.threads {
        c.args(["--threads", &t.to_string()]);
    }
    let status = c.status().map_err(|e| format!("cannot run compiled program: {}", e))?;
    let _ = fs::remove_dir_all(&tmp);
    Ok(status.code().unwrap_or(1))
}

fn cmd_build(args: &[String]) -> Result<i32, CliErr> {
    let o = parse_opts(args)?;
    let out = o.out.as_deref().ok_or("build requires -o <out>")?;
    let (sm, _) = specialized(&o)?;
    compile_program(&sm, out)?;
    println!("wrote {}", out.display());
    Ok(0)
}

fn cmd_net(args: &[String]) -> Result<i32, CliErr> {
    let (sm, reports) = specialized(&parse_opts(args)?)?;
    println!("{:<20} {:>9} {:>6} {:>5} {:>5} {:>7} {:>7}", "function", "rewrites", "calls", "ops", "evald", "before", "after");
    for r in &reports {
        println!("{:<20} {:>9} {:>6} {:>5} {:>5} {:>7} {:>7}", r.name, r.rewrites, r.calls_kept, r.ops_kept, r.calls_evaluated, r.size_before, r.size_after);
    }
    if std::env::var_os("MITHRIL_NET_CORE").is_some() {
        for f in &sm.fns {
            println!("{} = {:?}", f.name, f.body);
        }
    }
    Ok(0)
}

fn cmd_prove(args: &[String]) -> Result<i32, CliErr> {
    let o = parse_opts(args)?;
    let (_, reports) = front(&o.file)?;
    for r in reports.iter().filter(|r| !r.acc.is_empty()) {
        let verdict = if r.proven { "proven" } else { "declined" };
        println!("fold in `{}` (acc `{}`): {} — {}", r.func, r.acc, verdict, r.reason);
    }
    let lean_src = mithril_reassoc::lean_obligations(&reports);
    let out = o.file.parent().unwrap_or(Path::new(".")).join("obligations.lean");
    fs::write(&out, lean_src).map_err(|e| format!("cannot write {}: {}", out.display(), e))?;
    println!("wrote {}", out.display());
    match lean_binary() {
        Some(lean) => {
            let r = Command::new(&lean)
                .arg(&out)
                .output()
                .map_err(|e| format!("cannot run {}: {}", lean.display(), e))?;
            if r.status.success() {
                println!("lean: check passed");
                Ok(0)
            } else {
                eprintln!("lean: check FAILED\n{}", String::from_utf8_lossy(&r.stderr));
                Ok(1)
            }
        }
        None => {
            println!("lean: not found (~/.elan/bin/lean); check skipped");
            Ok(0)
        }
    }
}

fn lean_binary() -> Option<PathBuf> {
    let p = PathBuf::from(env::var_os("HOME")?).join(".elan/bin/lean");
    p.exists().then_some(p)
}

// -------------------------------------------------------------------- gpu

#[cfg(feature = "gpu")]
fn run_gpu(sm: &CoreModule) -> Result<i32, CliErr> {
    // the same lowering as the CPU program, printed for the device
    let cu = match mithril_gpu::emit_cuda(sm) {
        Ok(cu) => cu,
        Err(constant) => {
            println!("{constant}");
            return Ok(0);
        }
    };
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    let cache = target_dir().join("mithril-cache").join("gpu");
    fs::create_dir_all(&cache)?;
    let r = mithril_gpu::compile_and_run(&cu, boot, &cache).map_err(CliErr::Other)?;
    println!("{}", r.text);
    Ok(0)
}

#[cfg(not(feature = "gpu"))]
fn run_gpu(_sm: &CoreModule) -> Result<i32, CliErr> {
    eprintln!("gpu support not built; rebuild with --features gpu");
    Ok(1)
}
