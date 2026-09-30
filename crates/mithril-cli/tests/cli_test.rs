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

#[test]
fn run_gpu_without_feature_reports_and_exits_1() {
    let out = mithril(&["run", fixture("fact_while.py").to_str().unwrap(), "--gpu"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("gpu support not built; rebuild with --features gpu"),
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
    for flag in ["--image", "--stats"] {
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
