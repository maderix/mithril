//! Task 7 acceptance tests: golden pipeline tests for the dual-mode Rust
//! emitter. For each fixture the test runs parse -> analyze -> desugar ->
//! build -> reduce -> emit_rust, compiles the generated main.rs with rustc
//! against a pre-built mithril_rt rlib, executes it, and asserts stdout
//! equals the `eval_core` oracle. The parallel fixtures additionally assert
//! `--threads 8` output equals `--threads 1`.

use mithril_codegen::{emit_rust, fmt_val};
use mithril_front::core::CoreModule;
use mithril_front::{desugar, eval_core};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

// ---- build helper: compile mithril_rt (and deps) once ----

fn ws_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| ws_root().join("target"))
}

static BUILD_RT: Once = Once::new();

/// Returns (libmithril_rt.rlib path, deps dir for -L).
fn rt_libs() -> (PathBuf, PathBuf) {
    BUILD_RT.call_once(|| {
        let st = Command::new("cargo")
            .args(["build", "-p", "mithril-rt", "--release"])
            .current_dir(ws_root())
            .status()
            .expect("failed to run cargo build -p mithril-rt");
        assert!(st.success(), "cargo build -p mithril-rt failed");
    });
    let td = target_dir();
    let rlib = td.join("release/libmithril_rt.rlib");
    assert!(rlib.exists(), "missing {}", rlib.display());
    (rlib, td.join("release/deps"))
}

// ---- pipeline ----

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e))
}

/// parse -> analyze -> desugar -> build -> reduce(fuel) -> emit_rust.
fn pipeline(src: &str, reduce_fuel: u64) -> (CoreModule, String) {
    let mut m = mithril_front::parse(src).unwrap_or_else(|d| panic!("parse: line {}: {}", d.line, d.msg));
    let _reports = mithril_reassoc::analyze(&mut m);
    let cm = desugar(&m).unwrap_or_else(|d| panic!("desugar: line {}: {}", d.line, d.msg));
    let mut net = mithril_net::build(&cm);
    let _ = mithril_net::reduce(&mut net, &cm, reduce_fuel);
    let rs = emit_rust(&cm, &net);
    (cm, rs)
}

fn oracle(cm: &CoreModule) -> String {
    // eval_core recurses once per loop iteration; give it a deep stack.
    let cm = cm.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || fmt_val(&eval_core(&cm, cm.main, &[])))
        .unwrap()
        .join()
        .unwrap()
}

/// Compile generated main.rs into `dir` and return the binary path.
fn compile(rs: &str, name: &str) -> PathBuf {
    let (rlib, deps) = rt_libs();
    let dir = std::env::temp_dir().join(format!("mithril-codegen-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    let main_rs = dir.join("main.rs");
    std::fs::write(&main_rs, rs).unwrap();
    let bin = dir.join("prog");
    let out = Command::new("rustc")
        .args(["--edition", "2021", "-O"])
        .arg(&main_rs)
        .arg("--extern")
        .arg(format!("mithril_rt={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("failed to run rustc");
    assert!(
        out.status.success(),
        "rustc failed for {}:\n{}\n---- generated code ----\n{}",
        name,
        String::from_utf8_lossy(&out.stderr),
        rs
    );
    bin
}

fn run(bin: &Path, args: &[&str]) -> String {
    let out = Command::new(bin).args(args).output().expect("failed to run generated binary");
    assert!(
        out.status.success(),
        "generated binary failed ({:?}): {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

/// Full golden check at the given thread counts (all must equal the oracle).
fn golden(name: &str, reduce_fuel: u64, threads: &[&str]) {
    let src = fixture(name);
    let (cm, rs) = pipeline(&src, reduce_fuel);
    let want = oracle(&cm);
    let bin = compile(&rs, name.trim_end_matches(".py"));
    for t in threads {
        let got = run(&bin, &[t]);
        assert_eq!(got, want, "{} at --threads {}: stdout != eval_core", name, t);
    }
}

// ---- fixtures ----

#[test]
fn arith_const_folded_at_compile_time() {
    // Plenty of compile-time fuel: the net quiesces and emit_rust may emit a
    // constant program; output must still equal the oracle.
    let src = fixture("arith.py");
    let (cm, rs) = pipeline(&src, 100_000);
    assert!(!rs.contains("Engine::new"), "expected const-folded program for a quiesced net");
    let want = oracle(&cm);
    let bin = compile(&rs, "arith_const");
    assert_eq!(run(&bin, &[]), want);
}

#[test]
fn arith_runtime_path() {
    // Zero compile-time fuel forces the full engine-backed program.
    golden("arith.py", 0, &["1"]);
}

#[test]
fn fact_while_matches_oracle() {
    // spin(9000) needs > 4096 loop iterations: exercises the tail-recursive
    // dive loop's suspend (spawn pending call) / resume path.
    golden("fact_while.py", 0, &["1", "4"]);
}

#[test]
fn tree_sum_matches_oracle() {
    // mk(12, .) exceeds the default fuel: exercises the rule-form expansion
    // (records at fork points) plus dives near the leaves.
    golden("tree_sum.py", 0, &["1", "8"]);
}

#[test]
fn ctor_and_tuple_result_printing() {
    let src = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def main():
    return (Node(Leaf(1 + 2), Leaf(4)), 7, (8, 9))
";
    let (cm, rs) = pipeline(src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "ctor_print");
    assert_eq!(run(&bin, &["1"]), want);
}

#[test]
fn fold_sum_parallel_equals_sequential() {
    let src = fixture("fold_sum.py");
    let (cm, rs) = pipeline(&src, 0);
    // The fold must actually be proven, and the par-fold shape emitted.
    assert!(
        cm.fns.iter().any(|f| f.fold.as_ref().is_some_and(|fi| fi.proven)),
        "fold_sum.py: no proven fold in CoreModule"
    );
    assert!(rs.contains("par-fold"), "expected chunked par_fold emission");
    let want = oracle(&cm);
    let bin = compile(&rs, "fold_sum");
    let t1 = run(&bin, &["1"]);
    let t8 = run(&bin, &["8"]);
    assert_eq!(t1, want, "fold_sum --threads 1 != oracle");
    assert_eq!(t8, t1, "fold_sum --threads 8 != --threads 1");
}

#[test]
fn fold_sum_masked_parallel_equals_sequential() {
    let src = fixture("fold_sum_masked.py");
    let (cm, rs) = pipeline(&src, 0);
    assert!(
        cm.fns
            .iter()
            .any(|f| f.fold.as_ref().is_some_and(|fi| fi.proven
                && fi.combiner == mithril_front::core::Combiner::WrapAdd32)),
        "fold_sum_masked.py: no proven WrapAdd32 fold in CoreModule"
    );
    assert!(rs.contains("par-fold"), "expected chunked par_fold emission");
    let want = oracle(&cm);
    let bin = compile(&rs, "fold_sum_masked");
    let t1 = run(&bin, &["1"]);
    let t8 = run(&bin, &["8"]);
    assert_eq!(t1, want, "fold_sum_masked --threads 1 != oracle");
    assert_eq!(t8, t1, "fold_sum_masked --threads 8 != --threads 1");
}

#[test]
fn fib_naive_parallel_equals_sequential() {
    let src = fixture("fib_naive.py");
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "fib_naive");
    let t1 = run(&bin, &["1"]);
    let t8 = run(&bin, &["8"]);
    assert_eq!(t1, want, "fib_naive --threads 1 != oracle");
    assert_eq!(t8, t1, "fib_naive --threads 8 != --threads 1");
}

#[test]
fn small_fuel_still_correct() {
    // fuel=argv[2]; a tiny budget forces heavy suspension traffic.
    let src = fixture("fib_naive.py");
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "fib_small_fuel");
    assert_eq!(run(&bin, &["4", "16"]), want, "fib_naive --threads 4 fuel 16 != oracle");
}
