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

const USAGE: &str = "usage: mithril <run|build|net|prove|oracle> f.py [--threads N] [--gpu] [-o out] [--image out.ppm] [--stats out.json] | mithril exec <artefact>";

fn dispatch(args: &[String]) -> Result<i32, CliErr> {
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
    /// `run`: write the result as an image (see `write_image`)
    image: Option<PathBuf>,
    /// `run`: write timings and sizes as JSON
    stats: Option<PathBuf>,
}

fn parse_opts(args: &[String]) -> Result<Opts, CliErr> {
    let (mut file, mut threads, mut gpu, mut out) = (None, None, false, None);
    let (mut image, mut stats) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let path = |i: &mut usize, what: &str| -> Result<PathBuf, CliErr> {
            *i += 1;
            Ok(PathBuf::from(args.get(*i).ok_or(format!("{what} needs a path"))?))
        };
        match args[i].as_str() {
            "--image" => image = Some(path(&mut i, "--image")?),
            "--stats" => stats = Some(path(&mut i, "--stats")?),
            "--threads" => {
                i += 1;
                let v = args.get(i).ok_or("--threads needs a value")?;
                let n: usize = v.parse().map_err(|_| format!("--threads: not a count: '{}'", v))?;
                threads = Some(n.max(1));
            }
            "--gpu" => gpu = true,
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
    Ok(Opts { file: file.ok_or("missing input file")?, threads, gpu, out, image, stats })
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
    let ran = if o.gpu {
        run_gpu(&sm, front)?
    } else {
        let tmp = make_temp_dir()?;
        let bin = tmp.join("prog");
        let tc = std::time::Instant::now();
        compile_program(&sm, &bin)?;
        let compile = tc.elapsed().as_secs_f64();
        let mut c = Command::new(&bin);
        if let Some(t) = o.threads {
            c.args(["--threads", &t.to_string()]);
        }
        if o.image.is_none() && o.stats.is_none() {
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
    let dims = match &o.image {
        Some(path) => {
            let (w, h) = write_image(path, &ran.text)?;
            println!("wrote {} ({w}x{h})", path.display());
            Some((w, h))
        }
        None => {
            println!("{}", ran.text);
            None
        }
    };
    if let Some(path) = &o.stats {
        write_stats(path, &o, &ran, dims)?;
    }
    Ok(0)
}

/// The ints of a printed value in order, constructor names skipped.
fn ints_of(text: &str) -> Result<Vec<i64>, CliErr> {
    let b = text.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        let start = i;
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
        } else if b[i].is_ascii_digit() || (b[i] == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if matches!(b.get(i), Some(b'.' | b'e' | b'E')) {
                return Err("an image holds ints; the value has a float".into());
            }
            out.push(text[start..i].parse::<i64>().map_err(|e| format!("image value {}: {e}", &text[start..i]))?);
        } else {
            i += 1;
        }
    }
    Ok(out)
}

/// `--image`: the value is `(width, height, pixels)`, pixels any nesting of
/// tuples or constructors read depth-first, each pixel 0xRRGGBB or an
/// `(r, g, b)` of 0..255. Written as binary PPM.
fn write_image(path: &Path, text: &str) -> Result<(usize, usize), CliErr> {
    let v = ints_of(text)?;
    let (w, h) = match v[..] {
        [w, h, ..] if w > 0 && h > 0 => (w as usize, h as usize),
        _ => return Err("--image: the value must be (width, height, pixels)".into()),
    };
    let px = &v[2..];
    let byte = |c: i64| c.clamp(0, 255) as u8;
    let rgb: Vec<u8> = if px.len() == w * h {
        px.iter().flat_map(|&p| [byte(p >> 16 & 255), byte(p >> 8 & 255), byte(p & 255)]).collect()
    } else if px.len() == 3 * w * h {
        px.iter().map(|&c| byte(c)).collect()
    } else {
        return Err(format!("--image: {w}x{h} needs {} pixels or {} channels; the value has {}", w * h, 3 * w * h, px.len()).into());
    };
    let mut f = format!("P6\n{w} {h}\n255\n").into_bytes();
    f.extend(rgb);
    fs::write(path, f).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok((w, h))
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
        build_gpu(&sm, out)?;
    } else {
        compile_program(&sm, out)?;
    }
    println!("wrote {}", out.display());
    Ok(0)
}

/// `mithril exec <artefact>`: run a program `build --gpu` compiled, with no
/// front end (what a timed run of a built program measures).
fn cmd_exec(args: &[String]) -> Result<i32, CliErr> {
    let path = Path::new(args.first().ok_or("exec needs the built artefact")?);
    let bytes = fs::read(path)?;
    if let Some(v) = bytes.strip_prefix(b"MITHRIL-CONST ") {
        println!("{}", String::from_utf8_lossy(v).trim());
        return Ok(0);
    }
    exec_gpu(path)
}

/// `mithril oracle f.py`: the reference interpreter's value of `main`
/// (the same front stages as `run`, then `eval_core`, printed by `fmt_val`).
fn cmd_oracle(args: &[String]) -> Result<i32, CliErr> {
    let (m, _) = front(&parse_opts(args)?.file)?;
    let cm = to_core(&m)?;
    // eval_core recurses once per loop iteration; give it a deep stack
    let t = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || mithril_codegen::fmt_val(&mithril_front::eval_core(&cm, cm.main, &[])))?;
    println!("{}", t.join().map_err(|_| "oracle: evaluation panicked")?);
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
fn run_gpu(sm: &CoreModule, front: f64) -> Result<Ran, CliErr> {
    // the same lowering as the CPU program, printed for the device
    let cu = match mithril_gpu::emit_cuda(sm) {
        Ok(cu) => cu,
        Err(constant) => return Ok(Ran { text: constant, front, compile: 0.0, run: 0.0, rounds: None }),
    };
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    let cache = target_dir().join("mithril-cache").join("gpu");
    fs::create_dir_all(&cache)?;
    let t0 = std::time::Instant::now();
    let cubin = mithril_gpu::compile_to_cubin(&cu, &cache).map_err(CliErr::Other)?;
    let compile = t0.elapsed().as_secs_f64();
    mithril_gpu::EXITING.store(true, std::sync::atomic::Ordering::Relaxed);
    let t1 = std::time::Instant::now();
    let r = mithril_gpu::run_cubin(&cubin, boot).map_err(CliErr::Other)?;
    let run = t1.elapsed().as_secs_f64();
    if std::env::var_os("MITHRIL_TIMING").is_some() {
        eprintln!("mithril: device compile-or-load {:.0} ms, run + readback {:.0} ms", compile * 1e3, run * 1e3);
    }
    Ok(Ran { text: r.text, front, compile, run, rounds: Some(r.rounds) })
}

#[cfg(feature = "gpu")]
fn build_gpu(sm: &CoreModule, out: &Path) -> Result<(), CliErr> {
    match mithril_gpu::emit_cuda(sm) {
        Err(constant) => fs::write(out, format!("MITHRIL-CONST {constant}\n"))?,
        Ok(cu) => {
            let cache = target_dir().join("mithril-cache").join("gpu");
            fs::create_dir_all(&cache)?;
            fs::copy(mithril_gpu::compile_to_cubin(&cu, &cache).map_err(CliErr::Other)?, out)?;
        }
    }
    Ok(())
}

#[cfg(feature = "gpu")]
fn exec_gpu(path: &Path) -> Result<i32, CliErr> {
    let boot = mithril_rt::Redex { a: 0, b: 0, aux: mithril_rt::ROOT };
    mithril_gpu::EXITING.store(true, std::sync::atomic::Ordering::Relaxed);
    let r = mithril_gpu::run_cubin(path, boot).map_err(CliErr::Other)?;
    println!("{}", r.text);
    Ok(0)
}

#[cfg(not(feature = "gpu"))]
fn build_gpu(_sm: &CoreModule, _out: &Path) -> Result<(), CliErr> {
    Err("gpu support not built; rebuild with --features gpu".into())
}

#[cfg(not(feature = "gpu"))]
fn exec_gpu(_path: &Path) -> Result<i32, CliErr> {
    Err("gpu support not built; rebuild with --features gpu".into())
}

#[cfg(not(feature = "gpu"))]
fn run_gpu(_sm: &CoreModule, _front: f64) -> Result<Ran, CliErr> {
    Err("gpu support not built; rebuild with --features gpu".into())
}
