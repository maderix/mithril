//! `mithril` — CLI driver over the front/reassoc/net/codegen crates.
//!
//! Subcommands:
//! - `mithril run f.py [--threads N] [--gpu]` — full pipeline, execute.
//! - `mithril build f.py -o out`              — same, binary copied to `out`.
//! - `mithril net f.py`                       — print the reduced residual net.
//! - `mithril prove f.py`                     — write + check obligations.lean.
//! - `mithril oracle f.py`                    — print `eval_core` of main (the reference interpreter).
//!
//! Any `Diag` exits 1 with `line <n>: <msg>` on stderr. The compiled
//! `mithril_rt` rlib (and its `mithril_core` dep) is cached under
//! `target/mithril-cache/`, keyed by `rustc -V`.

use mithril_front::ast::Module;
use mithril_core::sink::{Leaf, Leaves, Sink};
use mithril_front::core::CoreModule;
use mithril_front::Diag;
use mithril_reassoc::FoldReport;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Fuel for the compile-time net reducer.
use mithril_net::REDUCE_FUEL;

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
    // one cooperative launch on one stream needs one hardware work queue
    // (the driver builds 8 by default; context setup halves). Set before
    // any thread or CUDA call exists.
    if env::var_os("CUDA_DEVICE_MAX_CONNECTIONS").is_none() {
        env::set_var("CUDA_DEVICE_MAX_CONNECTIONS", "1");
    }
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

const USAGE: &str = "usage: mithril <run|build|net|prove|oracle> f.py [--threads N] [--gpu] [--coop] [--metal] [-o out] [--image out.ppm | --raw out.bin] [--stats out.json] | mithril exec <artefact>... [--image out.ppm | --raw out.bin]";

const HELP: &str = "Mithril: compile a Python-subset program and run it on any number of threads or the GPU.

  mithril run f.py [--threads N] [--gpu]   compile and run (1 thread unless --threads); prints main()'s value
  mithril run f.py --image out.ppm         main() returns (width, height, pixels): write the image
  mithril run f.py --raw out.bin           write every number of main()'s value as 8 bytes
  mithril run f.py --coop --threads N      the CPU and the GPU share the program's large folds
  mithril run f.py --metal                 large proven folds run on the Metal GPU (macOS;
                                           also build --metal, then ./prog --metal)
  mithril oracle f.py                      run on the reference interpreter (slow: small inputs)
  mithril build f.py -o prog [--gpu]       compile to an executable (run it: ./prog --threads N)
  mithril exec prog                        run a program built with --gpu
  mithril exec a b c                       run several, paying the device setup once
  mithril net f.py                         what compile-time reduction did to each function
  mithril prove f.py                       prove the program's parallel folds (Lean obligations)

The answer is the same on every thread count and on the GPU.";

fn dispatch(args: &[String]) -> Result<i32, CliErr> {
    if matches!(args.first().map(String::as_str), Some("-h" | "--help" | "help")) {
        println!("{HELP}\n\n{USAGE}");
        return Ok(0);
    }
    match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("build") => cmd_build(&args[1..]),
        Some("net") => cmd_net(&args[1..]),
        Some("prove") => cmd_prove(&args[1..]),
        Some("exec") => cmd_exec(&args[1..]),
        Some("oracle") => cmd_oracle(&args[1..]),
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
    /// `run`, `oracle`: write the result to a file (`--image`, `--raw`)
    sink: Option<Sink>,
    /// `run`: write timings and sizes as JSON
    stats: Option<PathBuf>,
    /// `run`: the CPU and the GPU share the program's large folds
    coop: bool,
    /// `run` (macOS): range launches on the Metal GPU
    metal: bool,
}

fn parse_opts(args: &[String]) -> Result<Opts, CliErr> {
    let (mut file, mut threads, mut gpu, mut out) = (None, None, false, None);
    let (mut sink, mut stats, mut coop, mut metal) = (None, None, false, false);
    let mut i = 0;
    while i < args.len() {
        let path = |i: &mut usize, what: &str| -> Result<PathBuf, CliErr> {
            *i += 1;
            Ok(PathBuf::from(args.get(*i).ok_or(format!("{what} needs a path"))?))
        };
        match args[i].as_str() {
            f @ ("--image" | "--raw") => sink = Sink::from_flag(f, &path(&mut i, f)?.to_string_lossy()),
            "--stats" => stats = Some(path(&mut i, "--stats")?),
            "--threads" => {
                i += 1;
                let v = args.get(i).ok_or("--threads needs a value")?;
                let n: usize = v.parse().map_err(|_| format!("--threads: not a count: '{}'", v))?;
                threads = Some(n.max(1));
            }
            "--gpu" => gpu = true,
            "--coop" => coop = true,
            "--metal" => metal = true,
            "-o" => out = Some(path(&mut i, "-o")?),
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
    Ok(Opts { file: file.ok_or("missing input file")?, threads, gpu, out, sink, stats, coop, metal })
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
    let core = to_core(&m)?;
    // MITHRIL_NO_NET: compile the program as written, with no compile-time
    // rule firings (the A/B for what early reduction buys)
    if std::env::var_os("MITHRIL_NO_NET").is_some() {
        return Ok((core, Vec::new()));
    }
    Ok(mithril_net::specialize(&core, REDUCE_FUEL))
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

/// Where compiled device programs are cached (`MITHRIL_GPU_CACHE`, else
/// under the target directory).
#[cfg(cuda)]
fn gpu_cache() -> PathBuf {
    env::var_os("MITHRIL_GPU_CACHE").map(PathBuf::from).unwrap_or_else(|| target_dir().join("mithril-cache").join("gpu"))
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
    c.args(["--edition", "2021", "-C", "opt-level=3", "--crate-type", "rlib", "--crate-name", name])
        .arg(src)
        .arg("--out-dir")
        .arg(out_dir)
        .arg("-L")
        .arg(out_dir);
    for dep in ["mithril_core", "mithril_metal"] {
        let lib = out_dir.join(format!("lib{dep}.rlib"));
        if name != dep && lib.exists() {
            c.arg("--extern").arg(format!("{dep}={}", lib.display()));
        }
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
    for c in ["mithril-core/src", "mithril-core/device", "mithril-metal/src", "mithril-metal/msl", "mithril-rt/src"] {
        let src = crates_root.join(c);
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
    if cfg!(target_os = "macos") {
        rustc_rlib(&tmp, "mithril_metal", &crates_root.join("mithril-metal/src/lib.rs"))?;
    }
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
/// `metal`: the program carries its range leaves for the Metal GPU.
fn compile_program(cm: &CoreModule, out_bin: &Path, metal: bool) -> Result<(), CliErr> {
    let src = mithril_codegen::emit_rust_for(cm, metal);
    let cache = rt_cache_dir()?;
    let tmp = make_temp_dir()?;
    fs::write(tmp.join("main.rs"), src)?;
    let mut c = Command::new("rustc");
    c.args(["--edition", "2021", "-C", "opt-level=3"])
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

/// What a run printed and what it cost (seconds).
struct Ran {
    text: String,
    front: f64,
    compile: f64,
    run: f64,
    /// device rounds (grow sweeps and work phases)
    rounds: Option<u64>,
}

fn cmd_run(args: &[String]) -> Result<i32, CliErr> {
    let o = parse_opts(args)?;
    let t0 = std::time::Instant::now();
    let (sm, _) = specialized(&o)?;
    let front = t0.elapsed().as_secs_f64();
    if std::env::var_os("MITHRIL_TIMING").is_some() {
        eprintln!("mithril: front end + specialization {:.0} ms", front * 1e3);
    }
    if o.coop {
        return run_coop(&sm, &o);
    }
    let ran = if o.gpu {
        run_gpu(&sm, front, o.sink.as_ref())?
    } else {
        let tmp = make_temp_dir()?;
        let bin = tmp.join("prog");
        let tc = std::time::Instant::now();
        compile_program(&sm, &bin, o.metal)?;
        let compile = tc.elapsed().as_secs_f64();
        let mut c = Command::new(&bin);
        if let Some(t) = o.threads {
            c.args(["--threads", &t.to_string()]);
        }
        if o.metal {
            c.arg("--metal");
        }
        if let Some(sink) = &o.sink {
            c.args(sink.args());
        }
        if o.stats.is_none() {
            // nothing to post-process: the program prints as it runs
            let status = c.status().map_err(|e| format!("cannot run compiled program: {}", e))?;
            let _ = fs::remove_dir_all(&tmp);
            return Ok(status.code().unwrap_or(1));
        }
        let tr = std::time::Instant::now();
        let out = c.stderr(std::process::Stdio::inherit()).output().map_err(|e| format!("cannot run compiled program: {}", e))?;
        let run = tr.elapsed().as_secs_f64();
        let _ = fs::remove_dir_all(&tmp);
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() {
            println!("{text}");
            return Ok(out.status.code().unwrap_or(1));
        }
        Ran { text, front, compile, run, rounds: None }
    };
    println!("{}", ran.text);
    if let Some(path) = &o.stats {
        let dims = match &o.sink {
            Some(Sink::Image(p)) => ppm_dims(p),
            _ => None,
        };
        write_stats(path, &o, &ran, dims)?;
    }
    Ok(0)
}

/// Width and height from a written PPM's header.
fn ppm_dims(path: &Path) -> Option<(usize, usize)> {
    let mut head = [0u8; 64];
    let n = std::io::Read::read(&mut fs::File::open(path).ok()?, &mut head).ok()?;
    let text = String::from_utf8_lossy(&head[..n]);
    let mut words = text.split_ascii_whitespace().skip(1);
    Some((words.next()?.parse().ok()?, words.next()?.parse().ok()?))
}

/// The value printed as `text` written to `sink`. Only a result the
/// compiler reduced to a constant arrives as text; every run lane writes
/// its leaves directly.
fn write_constant(sink: &Sink, text: &str) -> Result<String, CliErr> {
    let mut l = Leaves::new();
    leaves_of(text)?.into_iter().for_each(|x| l.push(x));
    Ok(mithril_core::sink::write(sink, &l, 1)?)
}

/// The numbers of a printed value in order, constructor names skipped
/// (floats print in round-trip form, so the parse is exact).
fn leaves_of(text: &str) -> Result<Vec<Leaf>, CliErr> {
    let b = text.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        let start = i;
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            // inf, NaN
            match &text[start..i] {
                "inf" => out.push(Leaf::Float(if start > 0 && b[start - 1] == b'-' { f64::NEG_INFINITY } else { f64::INFINITY })),
                "NaN" => out.push(Leaf::Float(f64::NAN)),
                _ => {}
            }
        } else if b[i].is_ascii_digit() || (b[i] == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            let mut float = false;
            while i < b.len() && (b[i].is_ascii_digit() || matches!(b[i], b'.' | b'e' | b'E') || (matches!(b[i], b'-' | b'+') && matches!(b[i - 1], b'e' | b'E'))) {
                float |= matches!(b[i], b'.' | b'e' | b'E');
                i += 1;
            }
            let w = &text[start..i];
            out.push(if float {
                Leaf::Float(w.parse().map_err(|e| format!("value {w}: {e}"))?)
            } else {
                Leaf::Int(w.parse().map_err(|e| format!("value {w}: {e}"))?)
            });
        } else {
            i += 1;
        }
    }
    Ok(out)
}

/// A JSON string literal.
fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// `--stats`: what ran where, and what each stage cost.
fn write_stats(path: &Path, o: &Opts, r: &Ran, dims: Option<(usize, usize)>) -> Result<(), CliErr> {
    let opt = |v: Option<String>| v.unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\n  \"program\": {},\n  \"backend\": \"{}\",\n  \"threads\": {},\n  \"front_s\": {:.4},\n  \"compile_s\": {:.4},\n  \"run_s\": {:.4},\n  \"device_rounds\": {},\n  \"image\": {}\n}}\n",
        json_str(&o.file.display().to_string()),
        if o.gpu { "gpu" } else { "cpu" },
        opt(o.threads.filter(|_| !o.gpu).map(|t| t.to_string())),
        r.front,
        r.compile,
        r.run,
        opt(r.rounds.map(|n| n.to_string())),
        opt(dims.map(|(w, h)| format!("{{ \"width\": {w}, \"height\": {h} }}"))),
    );
    fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()).into())
}

fn cmd_build(args: &[String]) -> Result<i32, CliErr> {
    let o = parse_opts(args)?;
    let out = o.out.as_deref().ok_or("build requires -o <out>")?;
    let (sm, _) = specialized(&o)?;
    if o.gpu {
        // the device compile's recorded cost, whether compiled now or cached
        match build_gpu(&sm, out)? {
            Some((total, cicc, ptxas)) => println!("wrote {} (device compile {total:.1} s: cicc {cicc:.1} s, ptxas {ptxas:.1} s)", out.display()),
            None => println!("wrote {}", out.display()),
        }
    } else {
        compile_program(&sm, out, o.metal)?;
        println!("wrote {}", out.display());
    }
    Ok(0)
}

/// `mithril exec <artefact>`: run a program `build --gpu` compiled, with no
/// front end (what a timed run of a built program measures).
/// Several artefacts run one after another in one device session, so only
/// the first pays the device setup.
fn cmd_exec(args: &[String]) -> Result<i32, CliErr> {
    let (mut paths, mut sink) = (Vec::new(), None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a.starts_with('-') {
            let p = it.next().ok_or_else(|| format!("{a} needs a path"))?;
            if a == "--coop" {
                mithril_core::coop::open(Path::new(p))?;
                continue;
            }
            sink = Some(Sink::from_flag(a, p).ok_or_else(|| format!("unknown option '{a}'\n{USAGE}"))?);
        } else {
            paths.push(PathBuf::from(a));
        }
    }
    if paths.is_empty() {
        return Err("exec needs the built artefact".into());
    }
    if paths.len() > 1 && sink.is_some() {
        return Err("--image and --raw take one artefact".into());
    }
    if let [path] = &paths[..] {
        if let Some(text) = constant_of(path)? {
            match &sink {
                Some(sink) => println!("{}", write_constant(sink, &text)?),
                None => println!("{text}"),
            }
            return Ok(0);
        }
        return exec_gpu(path, sink.as_ref());
    }
    exec_gpu_session(&paths)
}

/// The value a `build --gpu` artefact of a compile-time constant holds.
fn constant_of(path: &Path) -> Result<Option<String>, CliErr> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(bytes.strip_prefix(b"MITHRIL-CONST ").map(|v| String::from_utf8_lossy(v).trim().to_string()))
}

/// `mithril oracle f.py`: the reference interpreter's value of `main`
/// (the same front stages as `run`, then `eval_core`, printed by `fmt_val`).
fn cmd_oracle(args: &[String]) -> Result<i32, CliErr> {
    // the oracle interprets the source as written: no fold or loop-split rewrite,
    // no compile-time reduction, so it checks those passes instead of sharing them
    let o = parse_opts(args)?;
    let path = o.file;
    let src = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let cm = to_core(&mithril_front::parse(&src)?)?;
    // eval_core recurses once per loop iteration (about 1 KiB each): a stack
    // reserved for millions of iterations, committed only as it is used
    let t = std::thread::Builder::new()
        .stack_size(1 << 33)
        .spawn(move || mithril_front::eval_core(&cm, cm.main, &[]))?;
    let v = t.join().map_err(|_| "oracle: evaluation panicked")?;
    match &o.sink {
        Some(sink) => {
            let mut out = Vec::new();
            mithril_codegen::val_leaves(&v, &mut out)?;
            let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
            let mut l = Leaves::new();
            out.into_iter().for_each(|x| l.push(x));
            println!("{}", mithril_core::sink::write(sink, &l, threads)?);
        }
        None => println!("{}", mithril_codegen::fmt_val(&v)),
    }
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

#[cfg(cuda)]
fn run_gpu(sm: &CoreModule, front: f64, sink: Option<&Sink>) -> Result<Ran, CliErr> {
    // the same lowering as the CPU program, printed for the device
    let cu = match mithril_gpu::emit_cuda(sm) {
        Ok(cu) => cu,
        Err(constant) => {
            let text = match sink {
                Some(sink) => write_constant(sink, &constant)?,
                None => constant,
            };
            return Ok(Ran { text, front, compile: 0.0, run: 0.0, rounds: None });
        }
    };
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    let cache = gpu_cache();
    fs::create_dir_all(&cache)?;
    let t0 = std::time::Instant::now();
    let cubin = mithril_gpu::compile_to_cubin(&cu, &cache).map_err(CliErr::Other)?;
    let compile = t0.elapsed().as_secs_f64();
    mithril_gpu::EXITING.store(true, std::sync::atomic::Ordering::Relaxed);
    let t1 = std::time::Instant::now();
    let r = mithril_gpu::run_cubin_to(&cubin, boot, sink).map_err(CliErr::Other)?;
    let run = t1.elapsed().as_secs_f64();
    if std::env::var_os("MITHRIL_TIMING").is_some() {
        eprintln!("mithril: device compile-or-load {:.0} ms, run + readback {:.0} ms", compile * 1e3, run * 1e3);
    }
    Ok(Ran { text: r.text, front, compile, run, rounds: Some(r.rounds) })
}

/// `run --coop`: the CPU program (its own process) and the GPU (this one)
/// run the program together, sharing its large folds through one
/// co-execution table. Each computes the whole value, so the two must
/// agree to the bit; the run fails if they do not.
#[cfg(cuda)]
fn run_coop(sm: &CoreModule, o: &Opts) -> Result<i32, CliErr> {
    let cu = match mithril_gpu::emit_cuda(sm) {
        Ok(cu) => cu,
        // reduced at compile time: nothing to share
        Err(constant) => {
            println!("{}", match &o.sink { Some(sink) => write_constant(sink, &constant)?, None => constant });
            return Ok(0);
        }
    };
    let tmp = make_temp_dir()?;
    let bin = tmp.join("prog");
    compile_program(sm, &bin, false)?;
    let cache = gpu_cache();
    fs::create_dir_all(&cache)?;
    let cubin = mithril_gpu::compile_to_cubin(&cu, &cache).map_err(CliErr::Other)?;
    let table = tmp.join("coop.table");
    mithril_core::coop::open(&table)?;
    // the GPU's copy of a file result goes beside it, then is compared
    let gpu_sink = o.sink.as_ref().map(|s| match s {
        Sink::Image(_) => Sink::Image(tmp.join("gpu.out")),
        Sink::Raw(_) => Sink::Raw(tmp.join("gpu.out")),
    });
    let mut c = Command::new(&bin);
    c.args(["--threads", &o.threads.unwrap_or(1).to_string(), "--coop"]).arg(&table);
    if let Some(sink) = &o.sink {
        c.args(sink.args());
    }
    let child = c.stdout(std::process::Stdio::piped()).spawn().map_err(|e| format!("cannot run compiled program: {e}"))?;
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    mithril_gpu::EXITING.store(true, std::sync::atomic::Ordering::Relaxed);
    let gpu = mithril_gpu::run_cubin_to(&cubin, boot, gpu_sink.as_ref());
    let out = child.wait_with_output()?;
    let cpu_text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        println!("{cpu_text}");
        return Ok(out.status.code().unwrap_or(1));
    }
    let gpu = gpu.map_err(CliErr::Other)?;
    let agree = match (&o.sink, &gpu_sink) {
        (Some(s), Some(g)) => fs::read(s.path())? == fs::read(g.path())?,
        _ => gpu.text == cpu_text,
    };
    let _ = fs::remove_dir_all(&tmp);
    if !agree {
        return Err(format!("--coop: the CPU and the GPU disagree\n  cpu: {cpu_text}\n  gpu: {}", gpu.text).into());
    }
    println!("{cpu_text}");
    Ok(0)
}

#[cfg(not(cuda))]
fn run_coop(_sm: &CoreModule, _o: &Opts) -> Result<i32, CliErr> {
    Err("--coop needs the GPU (Linux and the NVIDIA driver)".into())
}

/// Write the device artefact; returns its compile time (total, cicc,
/// ptxas seconds) when one was recorded.
#[cfg(cuda)]
fn build_gpu(sm: &CoreModule, out: &Path) -> Result<Option<(f64, f64, f64)>, CliErr> {
    match mithril_gpu::emit_cuda(sm) {
        Err(constant) => {
            fs::write(out, format!("MITHRIL-CONST {constant}\n"))?;
            Ok(None)
        }
        Ok(cu) => {
            let cache = gpu_cache();
            fs::create_dir_all(&cache)?;
            let cubin = mithril_gpu::compile_to_cubin(&cu, &cache).map_err(CliErr::Other)?;
            fs::copy(&cubin, out)?;
            Ok(mithril_gpu::compile_time(&cubin).map(|t| (t.total, t.cicc, t.ptxas)))
        }
    }
}

#[cfg(cuda)]
fn exec_gpu(path: &Path, sink: Option<&Sink>) -> Result<i32, CliErr> {
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    mithril_gpu::EXITING.store(true, std::sync::atomic::Ordering::Relaxed);
    let r = mithril_gpu::run_cubin_to(path, boot, sink).map_err(CliErr::Other)?;
    println!("{}", r.text);
    Ok(0)
}

#[cfg(cuda)]
fn exec_gpu_session(paths: &[PathBuf]) -> Result<i32, CliErr> {
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    let session = mithril_gpu::GpuSession::open().map_err(CliErr::Other)?;
    for path in paths {
        match constant_of(path)? {
            Some(text) => println!("{text}"),
            None => println!("{}", session.run(path, boot, None).map_err(CliErr::Other)?.text),
        }
    }
    Ok(0)
}

#[cfg(not(cuda))]
fn build_gpu(_sm: &CoreModule, _out: &Path) -> Result<Option<(f64, f64, f64)>, CliErr> {
    Err("gpu support not built (it needs Linux and the NVIDIA driver)".into())
}

#[cfg(not(cuda))]
fn exec_gpu(_path: &Path, _sink: Option<&Sink>) -> Result<i32, CliErr> {
    Err("gpu support not built (it needs Linux and the NVIDIA driver)".into())
}

#[cfg(not(cuda))]
fn exec_gpu_session(_paths: &[PathBuf]) -> Result<i32, CliErr> {
    Err("gpu support not built (it needs Linux and the NVIDIA driver)".into())
}

#[cfg(not(cuda))]
fn run_gpu(_sm: &CoreModule, _front: f64, _sink: Option<&Sink>) -> Result<Ran, CliErr> {
    Err("gpu support not built (it needs Linux and the NVIDIA driver)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printed_constants_parse_back_to_their_leaves() {
        let parse = |t: &str| leaves_of(t).ok().expect("parses");
        let v = parse("(C3(1, -2), [2.5, -1e-7, 3e20], C0(inf), (-inf, NaN), 1152921504606846976, x_1)");
        let want = [Leaf::Int(1), Leaf::Int(-2), Leaf::Float(2.5), Leaf::Float(-1e-7), Leaf::Float(3e20), Leaf::Float(f64::INFINITY), Leaf::Float(f64::NEG_INFINITY)];
        assert_eq!(v[..7], want);
        assert!(matches!(v[7], Leaf::Float(x) if x.is_nan()));
        assert_eq!(v[8..], [Leaf::Int(1 << 60)]);
        // round-trip: every float prints in a form that parses to the same bits
        for x in [0.1f64, -0.0, 1e-300, 123456.789, f64::MAX] {
            assert_eq!(parse(&format!("{x:?}")), [Leaf::Float(x)]);
        }
    }

    #[test]
    fn sink_dims_come_from_the_ppm_header() {
        let p = std::env::temp_dir().join(format!("mithril-dims-{}.ppm", std::process::id()));
        fs::write(&p, b"P6\n640 480\n255\n\x00").unwrap();
        assert_eq!(ppm_dims(&p), Some((640, 480)));
        let _ = fs::remove_file(p);
    }
}
