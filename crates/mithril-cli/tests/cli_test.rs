//! End-to-end tests driving the built `mithril` binary via std::process.
//!
//! `run`/`build` tests exercise mithril-codegen (built in parallel as
//! Task 7); they skip when that crate is absent or does not check yet.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_mithril")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn ws_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

/// Task 7 lands mithril-codegen in parallel; gate `run`/`build` tests on
/// it being present and checkable so this crate's suite stands alone.
fn codegen_ready() -> bool {
    if !ws_root().join("crates/mithril-codegen/src/lib.rs").exists() {
        eprintln!("SKIP: crates/mithril-codegen not present yet");
        return false;
    }
    let ok = Command::new("cargo")
        .args(["check", "-p", "mithril-codegen", "--quiet"])
        .current_dir(ws_root())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("SKIP: mithril-codegen does not cargo-check yet");
    }
    ok
}

fn mithril(args: &[&str]) -> Output {
    Command::new(bin()).args(args).output().expect("spawn mithril")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// GPU tests run where the backend is built in and `MITHRIL_GPU=1` says a
/// device and the nvcc image are there (as the device tests in mithril-gpu).
fn gpu_on() -> bool {
    cfg!(cuda) && std::env::var("MITHRIL_GPU").as_deref() == Ok("1")
}

fn fresh_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mithril-cli-test-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

// ------------------------------------------------------------------- run

#[test]
fn run_fact_while_prints_value() {
    if !codegen_ready() {
        return;
    }
    let out = mithril(&["run", fixture("fact_while.py").to_str().unwrap()]);
    assert!(out.status.success(), "run failed: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "3628800");
}

#[test]
fn run_threads_flag_gives_same_value() {
    if !codegen_ready() {
        return;
    }
    let f = fixture("fact_while.py");
    let a = mithril(&["run", f.to_str().unwrap()]);
    let b = mithril(&["run", f.to_str().unwrap(), "--threads", "4"]);
    assert!(a.status.success(), "run failed: {}", stderr(&a));
    assert!(b.status.success(), "run --threads 4 failed: {}", stderr(&b));
    assert_eq!(stdout(&a).trim(), stdout(&b).trim());
}

#[test]
fn run_fold_sum_prints_value() {
    if !codegen_ready() {
        return;
    }
    let out = mithril(&["run", fixture("fold_sum.py").to_str().unwrap()]);
    assert!(out.status.success(), "run failed: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "328350"); // sum i*i, i in 0..100
}

// ---------------------------------------------------------------- oracle

#[test]
fn oracle_prints_the_reference_interpreters_value() {
    for (f, want) in [("fact_while.py", "3628800"), ("fold_sum.py", "328350")] {
        let out = mithril(&["oracle", fixture(f).to_str().unwrap()]);
        assert!(out.status.success(), "oracle {f} failed: {}", stderr(&out));
        assert_eq!(stdout(&out), format!("{want}\n"), "oracle {f}");
    }
}

#[test]
fn oracle_prints_structured_values_like_the_compiled_program() {
    let d = fresh_dir("oracle_values");
    let f = write_prog(&d, "def main():\n    return (1, 2.5, array_new(2, 3), (0 - 5, 6))\n");
    let out = mithril(&["oracle", f.to_str().unwrap()]);
    assert!(out.status.success(), "oracle failed: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "(1, 2.5, [3, 3], (-5, 6))");
    if codegen_ready() {
        let run = mithril(&["run", f.to_str().unwrap()]);
        assert_eq!(stdout(&run), stdout(&out), "run and oracle print differently");
    }
}

#[test]
fn oracle_reports_front_end_errors_like_run() {
    let out = mithril(&["oracle", fixture("bad.py").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("line 2"), "stderr: {}", stderr(&out));
    let out = mithril(&["oracle", fixture("no_main.py").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("no main function"), "stderr: {}", stderr(&out));
    let out = mithril(&["oracle"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("missing input file"), "stderr: {}", stderr(&out));
}

// a CLI built with the GPU backend runs the program instead
#[cfg(not(cuda))]
#[test]
fn run_gpu_without_feature_reports_and_exits_1() {
    let out = mithril(&["run", fixture("fact_while.py").to_str().unwrap(), "--gpu"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("gpu support not built"),
        "stderr: {}",
        stderr(&out)
    );
}

// ----------------------------------------------------------------- build

#[test]
fn build_writes_runnable_binary() {
    if !codegen_ready() {
        return;
    }
    let dir = fresh_dir("build");
    let exe = dir.join("fact");
    let out = mithril(&[
        "build",
        fixture("fact_while.py").to_str().unwrap(),
        "-o",
        exe.to_str().unwrap(),
    ]);
    assert!(out.status.success(), "build failed: {}", stderr(&out));
    assert!(exe.exists(), "no binary at {}", exe.display());
    let run = Command::new(&exe).output().expect("spawn built binary");
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "3628800");
}

#[test]
fn build_without_output_path_fails() {
    let out = mithril(&["build", fixture("fact_while.py").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("-o"), "stderr: {}", stderr(&out));
}

// ------------------------------------------------------------------- net

#[test]
fn net_output_contains_redex() {
    let out = mithril(&["net", fixture("fact_while.py").to_str().unwrap()]);
    assert!(out.status.success(), "net failed: {}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("rewrites"), "no rewrite column in net output:\n{}", s);
    assert!(s.contains("main"), "no per-function report in net output:\n{}", s);
}

// ----------------------------------------------------------------- prove

#[test]
fn prove_writes_obligations_and_checks_when_lean_present() {
    let dir = fresh_dir("prove");
    let src = dir.join("fold_sum.py");
    fs::copy(fixture("fold_sum.py"), &src).unwrap();
    let out = mithril(&["prove", src.to_str().unwrap()]);
    let obligations = dir.join("obligations.lean");
    assert!(obligations.exists(), "prove wrote no obligations.lean: {}", stderr(&out));
    let text = fs::read_to_string(&obligations).unwrap();
    assert!(!text.is_empty());
    assert!(stdout(&out).contains("proven"), "stdout: {}", stdout(&out));
    let lean = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".elan/bin/lean"));
    if lean.map(|l| l.exists()).unwrap_or(false) {
        assert!(out.status.success(), "lean check failed: {}", stderr(&out));
        assert!(stdout(&out).contains("lean: check passed"), "stdout: {}", stdout(&out));
    } else {
        assert!(out.status.success(), "prove failed: {}", stderr(&out));
        assert!(stdout(&out).contains("check skipped"), "stdout: {}", stdout(&out));
    }
}

// ---------------------------------------------------------------- errors

#[test]
fn bad_program_exits_1_with_line_number() {
    let out = mithril(&["run", fixture("bad.py").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("line 2"), "stderr: {}", stderr(&out));
}

#[test]
fn missing_main_is_an_error() {
    let out = mithril(&["run", fixture("no_main.py").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("no main function"), "stderr: {}", stderr(&out));
}

#[test]
fn missing_file_exits_1() {
    let out = mithril(&["net", "does_not_exist.py"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("cannot read"), "stderr: {}", stderr(&out));
}

#[test]
fn unknown_subcommand_exits_1() {
    let out = mithril(&["frobnicate", "x.py"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("unknown subcommand"), "stderr: {}", stderr(&out));
}

#[test]
fn threads_flag_requires_count() {
    let out = mithril(&["run", fixture("fact_while.py").to_str().unwrap(), "--threads", "zebra"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("--threads"), "stderr: {}", stderr(&out));
}

// ------------------------------------------------------- --image / --stats

fn write_prog(dir: &Path, src: &str) -> PathBuf {
    let p = dir.join("prog.py");
    fs::write(&p, src).unwrap();
    p
}

#[test]
fn run_image_writes_packed_pixels_as_ppm() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("image-packed");
    let prog = write_prog(&d, "def main():\n    return (2, 2, ((16711680, 65280), (255, 16777215)))\n");
    let img = d.join("out.ppm");
    let out = mithril(&["run", prog.to_str().unwrap(), "--image", img.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), format!("wrote {} (2x2)", img.display()));
    let mut want = b"P6\n2 2\n255\n".to_vec();
    want.extend([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    assert_eq!(fs::read(&img).unwrap(), want);
}

#[test]
fn run_image_reads_rgb_triples_through_constructors() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("image-rgb");
    let src = "@data\nclass P:\n    Px: (r, g, b)\n\ndef main():\n    return (2, 1, (Px(1, 2, 3), Px(250, 300, -5)))\n";
    let prog = write_prog(&d, src);
    let img = d.join("out.ppm");
    let out = mithril(&["run", prog.to_str().unwrap(), "--image", img.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let mut want = b"P6\n2 1\n255\n".to_vec();
    // channels clamp to 0..255
    want.extend([1, 2, 3, 250, 255, 0]);
    assert_eq!(fs::read(&img).unwrap(), want);
}

#[test]
fn run_image_rejects_values_that_are_not_images() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("image-bad");
    let img = d.join("out.ppm");
    for (src, msg) in [
        ("def main():\n    return (2, 2, (1, 2, 3))\n", "needs 4 pixels or 12 channels; the value has 3"),
        ("def main():\n    return (1, 1, 0.5)\n", "the value has a float"),
        ("def main():\n    return 7\n", "must be (width, height, pixels)"),
        ("def main():\n    return (0, 1, 5)\n", "must be (width, height, pixels)"),
    ] {
        let prog = write_prog(&d, src);
        let out = mithril(&["run", prog.to_str().unwrap(), "--image", img.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(1), "{src}");
        assert!(stderr(&out).contains(msg), "{src}: {}", stderr(&out));
    }
}

#[test]
fn run_stats_records_backend_timings_and_image() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("stats");
    let prog = write_prog(&d, "def main():\n    return (1, 1, 255)\n");
    let (img, st) = (d.join("o.ppm"), d.join("s.json"));
    let out = mithril(&["run", prog.to_str().unwrap(), "--threads", "3", "--image", img.to_str().unwrap(), "--stats", st.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let j = fs::read_to_string(&st).unwrap();
    for field in ["\"backend\": \"cpu\"", "\"threads\": 3", "\"front_s\": ", "\"compile_s\": ", "\"run_s\": ", "\"device_rounds\": null", "\"width\": 1, \"height\": 1"] {
        assert!(j.contains(field), "missing {field} in {j}");
    }
    // without --image the value is printed and the image field is null
    let out = mithril(&["run", prog.to_str().unwrap(), "--stats", st.to_str().unwrap()]);
    assert_eq!(stdout(&out).trim(), "(1, 1, 255)");
    assert!(fs::read_to_string(&st).unwrap().contains("\"image\": null"));
}

#[test]
fn image_and_stats_need_a_path() {
    for flag in ["--image", "--raw", "--stats"] {
        let out = mithril(&["run", fixture("fact_while.py").to_str().unwrap(), flag]);
        assert_eq!(out.status.code(), Some(1));
        assert!(stderr(&out).contains(&format!("{flag} needs a path")), "{}", stderr(&out));
    }
}

#[test]
fn stats_program_path_is_valid_json() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("stats-json").join("a \"quoted\\ dir");
    fs::create_dir_all(&d).unwrap();
    let prog = write_prog(&d, "def main():\n    return 1\n");
    let st = d.join("s.json");
    let out = mithril(&["run", prog.to_str().unwrap(), "--stats", st.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let j = fs::read_to_string(&st).unwrap();
    assert!(j.contains("a \\\"quoted\\\\ dir"), "{j}");
}

// ------------------------------------------------------------ output sinks

/// Ints (one past i56), floats, constructors, tuples and arrays.
const MIXED: &str = "@data\nclass P:\n    Px: (r, g, b)\n\ndef main():\n    n = 3000\n    a = array_new(n, 0)\n    for i in range(n):\n        a = array_set(a, i, i * i - 7 * i)\n    f = array_new(4, 0.0)\n    x = 0.0 - 1.0\n    for i in range(4):\n        f = array_set(f, i, x)\n        x = x + 0.25\n    return (a, f, Px(1, 0 - 2, 3), 1152921504606846976, (0 - 5, 6.5))\n";

fn mixed_raw() -> Vec<u8> {
    let mut want: Vec<u8> = Vec::new();
    for i in 0..3000i64 {
        want.extend((i * i - 7 * i).to_le_bytes());
    }
    for i in 0..4 {
        want.extend((i as f64 * 0.25 - 1.0).to_bits().to_le_bytes());
    }
    for v in [1i64, -2, 3, 1 << 60, -5] {
        want.extend(v.to_le_bytes());
    }
    want.extend(6.5f64.to_bits().to_le_bytes());
    want
}

#[test]
fn raw_sink_writes_the_same_bytes_on_every_lane() {
    let d = fresh_dir("raw-lanes");
    let prog = write_prog(&d, MIXED);
    let want = mixed_raw();
    let mut lanes: Vec<(&str, Vec<&str>)> = vec![("oracle", vec!["oracle"])];
    if codegen_ready() {
        lanes.push(("t1", vec!["run"]));
        lanes.push(("t4", vec!["run", "--threads", "4"]));
    }
    if gpu_on() {
        lanes.push(("gpu", vec!["run", "--gpu"]));
    }
    for (name, args) in lanes {
        let out_path = d.join(format!("{name}.bin"));
        let mut a = args.clone();
        a.extend([prog.to_str().unwrap(), "--raw", out_path.to_str().unwrap()]);
        let out = mithril(&a);
        assert_eq!(out.status.code(), Some(0), "{name}: {}", stderr(&out));
        assert_eq!(stdout(&out).trim(), format!("wrote {} ({} values)", out_path.display(), want.len() / 8), "{name}");
        assert_eq!(fs::read(&out_path).unwrap(), want, "{name}");
    }
}

#[test]
fn a_built_program_takes_the_sink_flags_itself() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("raw-built");
    let prog = write_prog(&d, MIXED);
    let exe = d.join("prog");
    let out = mithril(&["build", prog.to_str().unwrap(), "-o", exe.to_str().unwrap()]);
    assert!(out.status.success(), "build failed: {}", stderr(&out));
    for t in ["1", "8"] {
        let raw = d.join(format!("t{t}.bin"));
        let run = Command::new(&exe).args(["--threads", t, "--raw", raw.to_str().unwrap()]).output().unwrap();
        assert!(run.status.success(), "{}", stderr(&run));
        assert_eq!(fs::read(&raw).unwrap(), mixed_raw(), "t{t}");
    }
    // a failed write is an error with a message, not a panic
    let run = Command::new(&exe).args(["--raw", d.join("no/such/dir.bin").to_str().unwrap()]).output().unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert!(stderr(&run).contains("cannot write"), "{}", stderr(&run));
}

#[test]
fn oracle_image_matches_the_compiled_image() {
    let d = fresh_dir("image-oracle");
    let src = "@data\nclass P:\n    Px: (r, g, b)\n\ndef main():\n    return (2, 1, (Px(1, 2, 3), Px(250, 300, 0 - 5)))\n";
    let prog = write_prog(&d, src);
    let img = d.join("o.ppm");
    let out = mithril(&["oracle", prog.to_str().unwrap(), "--image", img.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let mut want = b"P6\n2 1\n255\n".to_vec();
    want.extend([1, 2, 3, 250, 255, 0]);
    assert_eq!(fs::read(&img).unwrap(), want);
}

#[test]
fn a_closure_result_cannot_be_written() {
    let d = fresh_dir("raw-closure");
    let prog = write_prog(&d, "def main():\n    return lambda x: x + 1\n");
    let out = mithril(&["oracle", prog.to_str().unwrap(), "--raw", d.join("c.bin").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("closure"), "{}", stderr(&out));
}

/// A program the compiler reduces to a constant reaches the sink as text.
#[test]
fn a_constant_gpu_artefact_writes_the_same_bytes() {
    if !gpu_on() {
        return;
    }
    let d = fresh_dir("raw-const");
    let prog = write_prog(&d, "def main():\n    return (3, 0 - 4, 2.5, 1e-300, 1152921504606846976)\n");
    let art = d.join("prog.gpu");
    let out = mithril(&["build", prog.to_str().unwrap(), "--gpu", "-o", art.to_str().unwrap()]);
    assert!(out.status.success(), "build failed: {}", stderr(&out));
    let (a, b) = (d.join("exec.bin"), d.join("oracle.bin"));
    let out = mithril(&["exec", art.to_str().unwrap(), "--raw", a.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let out = mithril(&["oracle", prog.to_str().unwrap(), "--raw", b.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
    let out = mithril(&["exec", art.to_str().unwrap(), "--threads", "2"]);
    assert_eq!(out.status.code(), Some(1));
}

/// Several artefacts in one `exec` print what each prints alone.
#[test]
fn exec_runs_several_artefacts_in_one_session() {
    if !gpu_on() {
        return;
    }
    let d = fresh_dir("exec-session");
    let srcs = ["def main():\n    return 7\n", MIXED, "def fib(n):\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n\ndef main():\n    return fib(array_len(array_new(20, 0)))\n"];
    let mut arts = Vec::new();
    for (i, src) in srcs.iter().enumerate() {
        let sub = d.join(format!("p{i}"));
        fs::create_dir_all(&sub).unwrap();
        let prog = write_prog(&sub, src);
        let art = sub.join("prog.gpu");
        let out = mithril(&["build", prog.to_str().unwrap(), "--gpu", "-o", art.to_str().unwrap()]);
        assert!(out.status.success(), "build failed: {}", stderr(&out));
        arts.push(art.to_str().unwrap().to_string());
    }
    let alone: String = arts.iter().map(|a| stdout(&mithril(&["exec", a]))).collect();
    let mut args = vec!["exec"];
    // repeats reuse the session's buffers and modules
    let order = [0, 1, 2, 1, 2, 0];
    order.iter().for_each(|&k| args.push(&arts[k]));
    let together = mithril(&args);
    assert_eq!(together.status.code(), Some(0), "stderr: {}", stderr(&together));
    let lines: Vec<String> = alone.lines().map(String::from).collect();
    let want: String = order.iter().map(|&k| format!("{}\n", lines[k])).collect();
    assert_eq!(stdout(&together), want);
    assert_eq!(lines[2], "6765");
    let out = mithril(&["exec", &arts[1], &arts[2], "--raw", d.join("x.bin").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("take one artefact"), "{}", stderr(&out));
}

// ------------------------------------------------------------ co-execution

/// subsetsum split by the take/skip choice of its first 7 weights: a sum
/// fold of 128 irregular, forking sub-searches (the oracle's count 1113931).
const SUBSET: &str = "def prng(x):\n    b = x ^ ((x << 13) & 4294967295)\n    d = b ^ (b >> 17)\n    return d ^ ((d << 5) & 4294967295)\n\n\
def weight(i):\n    return (prng((i + 1) * 2654435761 & 4294967295) & 65535) + 1\n\n\
def total_weight(n):\n    s = 0\n    for i in range(n):\n        s = s + weight(i)\n    return s\n\n\
def count(i, n, room):\n    if i == n:\n        return 1\n    skip = count(i + 1, n, room)\n    w = weight(i)\n    if w > room:\n        return skip\n    take = count(i + 1, n, room - w)\n    return (skip + take) & 4294967295\n\n\
def walk(j, i, k, n, room):\n    if i == k:\n        return count(k, n, room)\n    if (j >> i) & 1 == 1:\n        w = weight(i)\n        if w > room:\n            return 0\n        return walk(j, i + 1, k, n, room - w)\n    return walk(j, i + 1, k, n, room)\n\n\
def main():\n    n = 24\n    room = total_weight(n) // 3\n    s = 0\n    for j in range(0, 128):\n        s = (s + walk(j, 0, 7, n, room)) & 4294967295\n    return s\n";

/// The chunks `MITHRIL_COOP_STATS` reports in `stderr`, as `[lo, hi)`.
fn coop_chunks(stderr: &str) -> Vec<(i64, i64)> {
    stderr
        .lines()
        .filter_map(|l| l.split_once(" [")?.1.split_once(')').map(|(r, _)| r.to_string()))
        .map(|r| {
            let (a, b) = r.split_once(", ").unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        })
        .collect()
}

#[test]
fn coop_cpu_processes_share_an_irregular_fold_and_agree_with_the_oracle() {
    if !codegen_ready() {
        return;
    }
    let d = fresh_dir("coop-cpu");
    let prog = write_prog(&d, SUBSET);
    let oracle = mithril(&["oracle", prog.to_str().unwrap()]);
    assert_eq!(stdout(&oracle).trim(), "1113931");
    let exe = d.join("prog");
    assert!(mithril(&["build", prog.to_str().unwrap(), "-o", exe.to_str().unwrap()]).status.success());
    for (round, threads) in [vec!["4"], vec!["2", "3"], vec!["1", "2", "4"]].iter().enumerate() {
        let table = d.join(format!("t{round}"));
        let kids: Vec<_> = threads
            .iter()
            .map(|t| {
                Command::new(&exe)
                    .args(["--threads", t, "--coop", table.to_str().unwrap()])
                    .env("MITHRIL_COOP_STATS", "1")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let mut chunks = Vec::new();
        for k in kids {
            let o = k.wait_with_output().unwrap();
            assert!(o.status.success(), "{}", stderr(&o));
            assert_eq!(stdout(&o).trim(), "1113931", "threads {threads:?}");
            chunks.extend(coop_chunks(&stderr(&o)));
        }
        // whoever ran what, the chunks are disjoint and cover the fold once
        chunks.sort();
        assert_eq!(chunks.first().map(|c| c.0), Some(0), "{chunks:?}");
        assert_eq!(chunks.last().map(|c| c.1), Some(128), "{chunks:?}");
        assert!(chunks.windows(2).all(|w| w[0].1 == w[1].0), "{chunks:?}");
    }
}

#[test]
fn coop_runs_a_fold_with_a_non_int_argument_alone() {
    if !codegen_ready() {
        return;
    }
    // the same search with its room passed in a tuple: not offered
    let src = SUBSET
        .replace("def walk(j, i, k, n, room):", "def walk(j, i, k, n, rt):\n    room = rt[0]")
        .replace("walk(j, i + 1, k, n, room - w)", "walk(j, i + 1, k, n, (room - w, 0))")
        .replace("walk(j, i + 1, k, n, room)", "walk(j, i + 1, k, n, (room, 0))")
        .replace("walk(j, 0, 7, n, room)", "walk(j, 0, 7, n, (room, 0))");
    let d = fresh_dir("coop-decline");
    let prog = write_prog(&d, &src);
    let exe = d.join("prog");
    assert!(mithril(&["build", prog.to_str().unwrap(), "-o", exe.to_str().unwrap()]).status.success());
    let o = Command::new(&exe).args(["--threads", "4", "--coop", d.join("t").to_str().unwrap()]).env("MITHRIL_COOP_STATS", "1").output().unwrap();
    assert_eq!(stdout(&o).trim(), "1113931", "{}", stderr(&o));
}

#[test]
fn run_coop_shares_folds_between_the_cpu_and_the_gpu() {
    if !gpu_on() {
        return;
    }
    let d = fresh_dir("coop-gpu");
    let prog = write_prog(&d, SUBSET);
    let out = mithril(&["run", prog.to_str().unwrap(), "--coop", "--threads", "4"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "1113931");
    // a file result: both write it, and the files must agree
    let (a, b) = (d.join("coop.bin"), d.join("plain.bin"));
    let out = mithril(&["run", prog.to_str().unwrap(), "--coop", "--threads", "4", "--raw", a.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(mithril(&["run", prog.to_str().unwrap(), "--raw", b.to_str().unwrap()]).status.success());
    assert_eq!(fs::read(&a).unwrap(), 1113931i64.to_le_bytes());
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}

// ------------------------------------------------------- float conformance

/// The bytes every backend writes for the float conformance program: every
/// f32 and f64 operation over subnormal, normal, huge, zero, infinite and NaN
/// cases (`mithril_core::float`: IEEE round to nearest, no fusing, canonical
/// NaN). The same constant on every machine: x86 and arm64 hosts, CUDA, Metal.
const FLOAT_CONFORMANCE_FNV: u64 = 0x634d_cadc_1878_89af;

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

#[test]
fn every_lane_writes_the_same_float_bits_on_every_machine() {
    let prog = fixture("float_conformance.py");
    let d = fresh_dir("float-conformance");
    let mut lanes: Vec<(&str, Vec<&str>)> = vec![("oracle", vec!["oracle"])];
    if codegen_ready() {
        lanes.push(("t1", vec!["run"]));
        lanes.push(("t4", vec!["run", "--threads", "4"]));
    }
    if gpu_on() {
        lanes.push(("gpu", vec!["run", "--gpu"]));
    }
    for (name, args) in lanes {
        let out = d.join(format!("{name}.bin"));
        let mut a = args.clone();
        a.extend([prog.to_str().unwrap(), "--raw", out.to_str().unwrap()]);
        let r = mithril(&a);
        assert_eq!(r.status.code(), Some(0), "{name}: {}", stderr(&r));
        let bytes = fs::read(&out).unwrap();
        assert_eq!(bytes.len(), 9660 * 8, "{name}");
        assert_eq!(fnv(&bytes), FLOAT_CONFORMANCE_FNV, "{name}: float bits differ from every other backend's");
    }
}

/// NaN bits are canonical wherever they leave the program, on every lane and
/// in both modes (the default canonicalizes the result; MITHRIL_STRICT_FLOAT
/// every f32 operation): a negated NaN, NaNs inside tuples, arrays of tuples
/// and nested arrays, a NaN inside a constructor (which makes the program
/// strict by itself), and the operations that never see bits (`==`, `int()`).
#[test]
fn nan_bits_are_canonical_wherever_they_leave_the_program() {
    let d = fresh_dir("nan-observed");
    let nan = "2143289344"; // 0x7fc00000
    let cases = [
        (
            "def main():\n    z = f32(array_len(array_new(0, 0)))\n    nan = z / z\n    a = array_new(3, (nan, f32(1.0)))\n    nest = array_new(2, array_new(2, -nan))\n    e = 0\n    if nan == nan:\n        e = 1\n    return (nan, -nan, a, nest, e, int(nan))\n",
            format!("({nan}, {nan}, [({nan}, 1065353216), ({nan}, 1065353216), ({nan}, 1065353216)], [[{nan}, {nan}], [{nan}, {nan}]], 0, 0)"),
        ),
        ("@data\nclass Box:\n    B: (v,)\n\ndef main():\n    z = f32(array_len(array_new(0, 0)))\n    return B(-(z / z))\n", format!("C0({nan})")),
    ];
    for (k, (src, want)) in cases.iter().enumerate() {
        let sub = d.join(format!("c{k}"));
        fs::create_dir_all(&sub).unwrap();
        let prog = write_prog(&sub, src);
        let mut lanes: Vec<Vec<&str>> = vec![vec!["oracle"]];
        if codegen_ready() {
            lanes.push(vec!["run"]);
            lanes.push(vec!["run", "--threads", "4"]);
        }
        if gpu_on() {
            lanes.push(vec!["run", "--gpu"]);
        }
        for strict in [false, true] {
            for lane in &lanes {
                let mut args = lane.clone();
                args.push(prog.to_str().unwrap());
                let mut c = Command::new(bin());
                c.args(&args);
                if strict {
                    c.env("MITHRIL_STRICT_FLOAT", "1");
                }
                let out = c.output().unwrap();
                assert_eq!(out.status.code(), Some(0), "case {k} {lane:?}: {}", stderr(&out));
                assert_eq!(stdout(&out).trim(), want, "case {k} {lane:?} strict={strict}");
            }
        }
    }
}

#[test]
fn the_float_conformance_bits_are_the_same_in_strict_mode() {
    let prog = fixture("float_conformance.py");
    let d = fresh_dir("float-strict");
    let out = d.join("strict.bin");
    let r = Command::new(bin()).args(["oracle", prog.to_str().unwrap(), "--raw", out.to_str().unwrap()]).env("MITHRIL_STRICT_FLOAT", "1").output().unwrap();
    assert_eq!(r.status.code(), Some(0), "{}", stderr(&r));
    assert_eq!(fnv(&fs::read(&out).unwrap()), FLOAT_CONFORMANCE_FNV);
    if codegen_ready() {
        let out = d.join("strict_t4.bin");
        let r = Command::new(bin()).args(["run", prog.to_str().unwrap(), "--threads", "4", "--raw", out.to_str().unwrap()]).env("MITHRIL_STRICT_FLOAT", "1").output().unwrap();
        assert_eq!(r.status.code(), Some(0), "{}", stderr(&r));
        assert_eq!(fnv(&fs::read(&out).unwrap()), FLOAT_CONFORMANCE_FNV);
    }
}

// ------------------------------------------------------------------ metal

/// The Metal lane (macOS): range launches on the GPU write the CPU's bytes.
/// The fixture's folds cover int sums (wrapping and 32-bit), a fill reading
/// a borrowed array, binary32 subnormals and guarded recursion, and one
/// fold whose recursion outgrows the device's stack (it falls back to the
/// CPU).
#[test]
fn the_metal_lane_writes_the_cpu_bytes() {
    if !cfg!(target_os = "macos") || !codegen_ready() {
        return;
    }
    let d = fresh_dir("metal");
    for name in ["metal_ranges.py"] {
        let prog = fixture(name);
        let mut bytes = Vec::new();
        for (lane, extra) in [("t1", vec![]), ("t4", vec!["--threads", "4"]), ("metal", vec!["--threads", "4", "--metal"])] {
            let out = d.join(format!("{name}.{lane}.bin"));
            let mut args = vec!["run", prog.to_str().unwrap(), "--raw", out.to_str().unwrap()];
            args.extend(extra);
            // (MITHRIL_METAL_TEST: the GPU takes part whatever it costs)
            let r = Command::new(bin()).args(&args).env("MITHRIL_METAL_TRACE", "1").env("MITHRIL_METAL_TEST", "1").output().unwrap();
            assert_eq!(r.status.code(), Some(0), "{name} {lane}: {}", stderr(&r));
            if lane == "metal" {
                // the GPU ran at least one fold (a request it completed)
                assert!(stderr(&r).lines().any(|l| l.starts_with("metal: fold") && l.ends_with("to the GPU")), "{name}: no fold ran on the GPU:\n{}", stderr(&r));
            }
            bytes.push((lane, fs::read(&out).unwrap()));
        }
        for (lane, b) in &bytes[1..] {
            assert!(*b == bytes[0].1, "{name}: {lane} differs from t1");
        }
    }
}
