//! Task 7 acceptance tests: golden pipeline tests for the dual-mode Rust
//! emitter. For each fixture the test runs parse -> analyze -> desugar ->
//! specialize -> emit_rust, compiles the generated main.rs with rustc
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

/// parse -> analyze -> desugar -> specialize (by the net rules) -> emit_rust.
fn pipeline(src: &str, reduce_fuel: u64) -> (CoreModule, String) {
    let mut m = mithril_front::parse(src).unwrap_or_else(|d| panic!("parse: line {}: {}", d.line, d.msg));
    let _reports = mithril_reassoc::analyze(&mut m);
    let cm = desugar(&m).unwrap_or_else(|d| panic!("desugar: line {}: {}", d.line, d.msg));
    let (sm, _) = mithril_net::specialize(&cm, reduce_fuel.max(1 << 20));
    let rs = emit_rust(&sm);
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

fn run_env(bin: &Path, args: &[&str], envs: &[(&str, &str)]) -> (String, String) {
    let mut c = Command::new(bin);
    c.args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = c.output().expect("failed to run generated binary");
    assert!(
        out.status.success(),
        "generated binary failed ({:?}, env {:?}): {}",
        args,
        envs,
        String::from_utf8_lossy(&out.stderr)
    );
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).trim_end().to_string(),
    )
}

fn run(bin: &Path, args: &[&str]) -> String {
    run_env(bin, args, &[]).0
}

/// `peak_cells=N` from the generated program's stderr stats line.
fn peak_cells(stderr: &str) -> usize {
    stderr
        .lines()
        .find_map(|l| l.strip_prefix("peak_cells="))
        .unwrap_or_else(|| panic!("no peak_cells line in stderr: {stderr:?}"))
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .expect("bad peak_cells value")
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
fn small_arena_tree_sum_and_fib() {
    // Hard gate for the linear cell discipline: a 2^16-cell arena would be
    // exhausted by leak-everything codegen (tree_sum alone allocates far
    // more than 2^16 cells across mk retries and rebuilds without freeing).
    let arena = &[("MITHRIL_NODES", "65536")];
    for (fx, threads) in [("tree_sum.py", "1"), ("tree_sum.py", "8"), ("fib_naive.py", "8")] {
        let src = fixture(fx);
        let (cm, rs) = pipeline(&src, 0);
        let want = oracle(&cm);
        let bin = compile(&rs, &format!("small-arena-{}", fx.trim_end_matches(".py")));
        let (got, _) = run_env(&bin, &[threads], arena);
        assert_eq!(got, want, "{fx} with MITHRIL_NODES=2^16 at --threads {threads}");
    }
}

#[test]
fn tree_sum_peak_cells_is_o_tree_size() {
    // mk(12) builds ~8k live cells (4096 Leaf + 4095 Node); with linear
    // freeing the arena footprint stays a small multiple of that (failed
    // dive attempts leak their partial allocations, bounded per attempt by
    // the fuel), nowhere near the total allocation count.
    let src = fixture("tree_sum.py");
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "tree-sum-peak");
    let (got, err) = run_env(&bin, &["1"], &[("MITHRIL_STATS", "1")]);
    assert_eq!(got, want);
    let peak = peak_cells(&err);
    assert!(
        peak < 48_000,
        "tree_sum peak_cells = {peak}, expected O(tree size) (< 48k), not O(total allocations)"
    );
}

#[test]
fn wide_live_tuple_loop() {
    // Defect D1 regression: a loop carrying 25 live locals (24 registers +
    // the counter) desugars into a state tuple wider than the old 15-ary
    // cap; the chained constructor encoding must round-trip it (nbody's
    // step loop carries 52). Also forces suspension traffic on the wide
    // state (500 iterations x 25 live values at small fuel).
    let src = fixture("wide_loop.py");
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "wide_loop");
    assert_eq!(run(&bin, &["1"]), want, "wide_loop --threads 1 != eval_core");
    assert_eq!(run(&bin, &["8"]), want, "wide_loop --threads 8 != eval_core");
    assert_eq!(run(&bin, &["4", "64"]), want, "wide_loop --threads 4 fuel 64 != eval_core");
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

// ---- tail recursion modulo cons ----

/// The generated dive form `d_<fid>` of the function named `name`.
fn dive_form<'a>(cm: &CoreModule, rs: &'a str, name: &str) -> &'a str {
    let fid = cm.fns.iter().position(|f| f.name == name).unwrap_or_else(|| panic!("no fn {name}"));
    let start = rs.find(&format!("fn d_{fid}(")).unwrap_or_else(|| panic!("no d_{fid} for {name}"));
    let end = rs[start..].find(&format!("fn x_{fid}(")).map(|e| start + e).unwrap_or(rs.len());
    &rs[start..end]
}

/// Oracle equality over thread counts and fuels (fuel 1 suspends at every
/// call and loop iteration, exercising the hole-fill records).
fn trmc_golden(name: &str) -> (CoreModule, String) {
    let src = fixture(name);
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, name.trim_end_matches(".py"));
    for t in ["1", "4", "16"] {
        assert_eq!(run(&bin, &[t]), want, "{name} --threads {t} (default fuel)");
        for fuel in ["1", "2", "7", "64"] {
            assert_eq!(run(&bin, &[t, fuel]), want, "{name} --threads {t} fuel {fuel}");
        }
    }
    (cm, rs)
}

#[test]
fn dependent_calls_wait_only_for_their_own_input() {
    // fork_chain: `Node(merge(bump(a)), merge(bump(b)))`. When bump(a)
    // suspends, merge(bump(a)) needs only its result: it waits behind a
    // pend-1 record whose parent is the pend-2 join of both branches, so
    // the independent branch does not gate it (a two-way split made the
    // whole merge run right-then-left, a chain of 2^d steps).
    let (cm, rs) = trmc_golden("fork_chain.py");
    let df = dive_form(&cm, &rs, "merge");
    let re = has_chained_pend1_record(df);
    assert!(re, "merge's capture has no dependent-chain record (pend 1, parent = the join record):\n{df}");
    // the CPU has one world: every callee gets the caller's budget through fork_fuel
    assert!(df.contains("fork_fuel(fuel)"), "merge's calls do not pass their budget through fork_fuel");
}

#[test]
fn a_tree_shared_by_many_parallel_forks_survives() {
    golden("shared_tree.py", 0, &["1", "16", "16", "16"]);
}

#[test]
fn split_shapes_and_chains_match_oracle() {
    // every split shape (a boxed value shared by D, P and J2; the three
    // two-way fallbacks) and a boxed chain, on every thread count and fuel
    trmc_golden("fork_split_shapes.py");
    trmc_golden("chain_boxed.py");
    trmc_golden("deep_leaves.py");
}

/// A one-slot record whose parent is a two-slot (join) record allocated
/// in the same capture: `let tJ: u32 = alloc_rec(ctx, .., 2u32, ..)` then
/// `alloc_rec(ctx, .., 1u32, .., rec_addr(tJ))`.
fn has_chained_pend1_record(s: &str) -> bool {
    s.match_indices(": u32 = alloc_rec(ctx, ").any(|(i, _)| {
        let call = &s[i..s[i..].find(");").map(|e| i + e).unwrap_or(s.len())];
        if !call.contains(", 2u32, ") {
            return false;
        }
        let name = s[..i].rsplit("let ").next().unwrap_or("").trim();
        let rest = &s[i..];
        rest.match_indices("alloc_rec(ctx, ").any(|(j, _)| {
            let c = &rest[j..rest[j..].find(");").map(|e| j + e).unwrap_or(rest.len())];
            c.contains(", 1u32, ") && c.ends_with(&format!("rec_addr({name})"))
        })
    })
}

#[test]
fn trmc_list_builders_match_oracle_under_suspension() {
    let (cm, rs) = trmc_golden("trmc_list.py");
    // direct TRMC and the delayed self call through branches both loop
    assert!(dive_form(&cm, &rs, "run_of").contains("hole_link("), "run_of is not a TRMC loop");
    assert!(dive_form(&cm, &rs, "build").contains("hole_link("), "build is not a TRMC loop");
    // run_of is a destination-passing callee and build appends through it
    let fid = cm.fns.iter().position(|f| f.name == "run_of").unwrap();
    assert!(rs.contains(&format!("fn dp_{fid}(")), "no destination-passing form for run_of");
    assert!(dive_form(&cm, &rs, "build").contains(&format!("dp_{fid}(ctx, fuel")), "build does not append via dp_run_of");
    // the doubly wrapped shape is not eligible and keeps plain recursion
    assert!(!dive_form(&cm, &rs, "build2").contains("th_head"), "build2 must not be TRMC");
}

#[test]
fn trmc_tree_builders_match_oracle_under_suspension() {
    let (cm, rs) = trmc_golden("trmc_tree.py");
    let d = dive_form(&cm, &rs, "swap_add");
    assert!(d.contains("hole_link("), "swap_add's second call is not a TRMC site");
    assert!(dive_form(&cm, &rs, "mk").contains("hole_link("), "mk's second call is not a TRMC site");
}

// ---- borrowed parameters ----

#[test]
fn a_lent_child_bound_by_a_branch_is_copied_into_its_binding() {
    // `p = lft(a)` binds a branch result that reads a lent tree; the
    // binding is owned (consumed by `leaf`, freed unused on one branch), so
    // the read is copied. Missing the copy freed the lender's subtree
    // (kmeans: a stack overflow on the corrupted stats tree).
    let (cm, rs) = pipeline(&fixture("lent_pick.py"), 100_000);
    let fid = cm.fns.iter().position(|f| f.name == "pick").unwrap();
    assert!(rs.contains(&format!("{fid} => {{\nlet r = d_{fid}(ctx, fuel, args[1]")), "pick: tree parameter is not borrowed");
    golden("lent_pick.py", 100_000, &["1", "4", "16"]);
    trmc_golden("lent_pick.py");
}

#[test]
fn borrowed_shared_tree_matches_oracle_under_suspension() {
    // ev/size/fit only read the tree, which fit shares across 40 calls:
    // they borrow it. Low fuel makes the borrowers suspend mid-tree, so the
    // capture-time references and the lender's release are exercised.
    let (cm, rs) = trmc_golden("borrow_eval.py");
    for name in ["ev", "size"] {
        let fid = cm.fns.iter().position(|f| f.name == name).unwrap();
        // the owned-argument entry releases the lent tree after the dive
        assert!(
            rs.contains(&format!("{fid} => {{\nlet r = d_{fid}(ctx, fuel, args[1]")),
            "{name}: tree parameter is not borrowed"
        );
    }
    // the match on a borrowed tree reads it without consuming it
    assert!(!dive_form(&cm, &rs, "ev").contains("consume2k(ctx, v0"), "ev consumes its borrowed tree");
}

#[test]
fn linear_list_is_not_borrowed() {
    // total's list is never shared: it keeps moving ownership (no refcount
    // on the type), so TRMC builders and the consumer stay linear.
    let (cm, rs) = pipeline(&fixture("trmc_list.py"), 0);
    let fid = cm.fns.iter().position(|f| f.name == "total").unwrap();
    assert!(rs.contains(&format!("            {fid} => d_{fid}(ctx, fuel")), "total borrows a linear list");
}

// ---- partial inlining of base cases ----

#[test]
fn base_case_wrappers_match_oracle_under_suspension() {
    let (cm, rs) = trmc_golden("fast_leaf.py");
    for name in ["zipsum", "inc"] {
        let fid = cm.fns.iter().position(|f| f.name == name).unwrap();
        assert!(rs.contains(&format!("fn q_{fid}(")), "{name} has no base-case wrapper");
        // recursive call sites go through the wrapper
        // (a fork site hands the callee `fork_fuel(fuel)`)
        let df = dive_form(&cm, &rs, name);
        assert!(df.contains(&format!("q_{fid}(ctx, fuel")) || df.contains(&format!("q_{fid}(ctx, fork_fuel(fuel)")), "{name} does not call q_{fid}");
    }
    // zipsum's leaf arm drops the owned, unmatched tree u
    let fid = cm.fns.iter().position(|f| f.name == "zipsum").unwrap();
    let q = &rs[rs.find(&format!("fn q_{fid}(")).unwrap()..];
    let q = &q[..q.find("\n}\n").unwrap()];
    assert!(q.contains("free_val(ctx, v1)"), "zipsum's base case leaks u");
}

// ---- compile-time unfolding, wrap elision, fuel in scalar code ----

#[test]
fn unfolding_and_wrap_elision_match_oracle() {
    let (cm, rs) = trmc_golden("unfold_wrap.py");
    // `rounds(3, 0, i)` has static control: unfolded, no call remains in loop
    let rounds = cm.fns.iter().position(|f| f.name == "rounds").unwrap();
    let collatz = cm.fns.iter().position(|f| f.name == "collatz").unwrap();
    let lp = cm.fns.iter().position(|f| f.name == "loop").unwrap();
    let s_loop = &rs[rs.find(&format!("fn s_{lp}(")).expect("loop is scalar")..];
    let s_loop = &s_loop[..s_loop.find("\n}\n").unwrap()];
    assert!(!s_loop.contains(&format!("s_{rounds}(")), "rounds(3, 0, i) was not unfolded");
    // collatz branches on a runtime value: still called
    assert!(s_loop.contains(&format!("s_{collatz}(")), "collatz must not be unfolded");
    // bitmix's loop (bounds through let-bound constants, runtime branch,
    // tuple state) unfolds: no loop helper remains in it
    let bm = &rs[rs.find(&format!("fn s_{}(", cm.fns.iter().position(|f| f.name == "bitmix").unwrap())).unwrap()..];
    let bm = &bm[..bm.find("\n}\n").unwrap()];
    assert!(!bm.contains("continue 'l") && !bm.contains("s_") || !bm.contains("__for"), "bitmix's loop was not unfolded");
    for f in cm.fns.iter().enumerate().filter(|(_, f)| f.name.starts_with("__for")) {
        assert!(!bm.contains(&format!("s_{}(", f.0)), "bitmix still calls its loop helper");
    }
    // ftree forks: it keeps a real (splittable) dive form, not a bridge
    let ft = cm.fns.iter().position(|f| f.name == "ftree").unwrap();
    assert!(dive_form(&cm, &rs, "ftree").contains(&format!("d_{ft}(ctx, fuel")), "ftree has no splittable dive form");
}

// ---- if-conversion, native scalar calls, 32-bit narrowing ----

#[test]
fn ifconv_native_calls_and_narrowing_match_oracle() {
    let (cm, rs) = trmc_golden("ifconv_native.py");
    let bins = cm.fns.iter().position(|f| f.name.starts_with("__for")).expect("loop helper");
    let s = &rs[rs.find(&format!("fn s_{bins}(")).expect("loop is scalar")..];
    let s = &s[..s.find("\nfn ").unwrap_or(s.len())];
    // the if/elif accumulator update became selects: one back-edge
    assert_eq!(s.matches("continue 'l").count(), 1, "if/elif in the loop was not if-converted");
    // masked arithmetic is computed in 32 bits
    assert!(s.contains("as u32).wrapping_mul("), "masked products are not narrowed");
    // the dive-form caller destructures the native tuple (no heap tuple)
    let leaf = dive_form(&cm, &rs, "leaf");
    let b = cm.fns.iter().position(|f| f.name == "bins").unwrap();
    assert!(leaf.contains(&format!(") = s_{b}(fuel")) || leaf.contains(&format!(") = s_{b}(ctx, fuel")), "leaf does not call bins natively");
    assert!(!leaf.contains("field(ctx"), "leaf reads its tuple from the heap");
    // a branch choosing between boxed subtrees is not if-converted
    assert_eq!(dive_form(&cm, &rs, "pick").matches("continue 'l").count(), 2, "pick's subtree choice became a select");
}

#[test]
fn forks_reached_from_scalar_callers_stay_splittable() {
    let (cm, rs) = trmc_golden("fork_reach.py");
    let id = |n: &str| cm.fns.iter().position(|f| f.name == n).unwrap();
    // the fork and its caller run in dive form (no native forms that would
    // hide the fork from the scheduler); the leaf is native
    assert!(!rs.contains(&format!("fn s_{}(", id("batch"))), "batch has a native form");
    assert!(!rs.contains(&format!("fn s_{}(", id("run"))), "run calls the fork natively");
    assert!(rs.contains(&format!("fn s_{}(", id("leafwork"))), "leafwork is not native");
}

// ---- ownership at projections and branch points ----

#[test]
fn projections_and_branches_release_exactly_once() {
    // A table carried through loops and read back: projecting the loop's
    // tuple result must move (not copy) a never-shared value, and a branch
    // that walks one subtree must release the other. Peak cells stay at one
    // table (~520) whatever the iteration count at one thread; a use after
    // free shows up as a wrong result or a crash at any thread count.
    let src = fixture("table_loop.py");
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "table_loop");
    for t in ["1", "16"] {
        for fuel in ["64", "4096"] {
            let (got, err) = run_env(&bin, &[t, fuel], &[("MITHRIL_STATS", "1")]);
            assert_eq!(got, want, "table_loop --threads {t} fuel {fuel}");
            // (with several workers, cells freed by one worker go to its own
            // free list, so the peak also reflects allocator migration)
            if t == "1" {
                assert!(peak_cells(&err) < 1024, "table_loop leaks: {err}");
            }
        }
    }
}

// ---- arrays ----

#[test]
fn arrays_match_oracle_in_place_and_shared() {
    let (cm, rs) = trmc_golden("arrays.py");
    // in-place updates and copy-on-write both live in arr_set
    assert!(rs.contains("arr_set(ctx"), "no array updates emitted");
    // an int array filled in a loop: native, unchecked in-place writes
    native_form(&cm, &rs, "fill");
    assert!(rs.contains("arr_set_u("), "int array writes are not native");
    // an array of lists keeps refcounted element handling in dive form
    let lp = cm.fns.iter().position(|f| f.name.starts_with("__for") && rs[rs.find(&format!("fn d_{}(", cm.fns.iter().position(|g| g.name == f.name).unwrap())).unwrap()..].split("\nfn ").next().unwrap().contains("arr_set(ctx")).expect("buckets' loop keeps refcounted updates");
    assert!(!rs.contains(&format!("fn s_{lp}(")), "boxed array loop went native");
    assert!(!rs.contains(&format!("fn s_{}(", cm.fns.iter().position(|f| f.name == "buckets").unwrap())), "boxed array went native");
}

// ---- native code over int arrays, native multi-value returns ----

/// The native form `s_<fid>` of the function named `name`.
fn native_form<'a>(cm: &CoreModule, rs: &'a str, name: &str) -> &'a str {
    let fid = cm.fns.iter().position(|f| f.name == name).unwrap_or_else(|| panic!("no fn {name}"));
    let start = rs.find(&format!("fn s_{fid}(")).unwrap_or_else(|| panic!("{name} is not native"));
    let end = rs[start..].find("\nfn ").map(|e| start + e).unwrap_or(rs.len());
    &rs[start..end]
}

#[test]
fn native_arrays_match_oracle_under_suspension() {
    let (cm, rs) = trmc_golden("native_arrays.py");
    let id = |n: &str| cm.fns.iter().position(|f| f.name == n).unwrap();
    // loops and helpers over int arrays run natively
    for f in ["step", "fill", "score", "walk", "upd2", "pick", "sum_pair", "swaps"] {
        native_form(&cm, &rs, f);
    }
    // writes in native code need no refcount check: arrays there are
    // linear and made unique where they enter (the bridge)
    let fill_loop = cm.fns.iter().position(|f| f.name.starts_with("__for")).unwrap();
    assert!(rs[rs.find(&format!("fn s_{fill_loop}(")).expect("fill's loop is native")..].contains("arr_set_u("), "native write still checks the refcount");
    assert!(dive_form(&cm, &rs, "fill").contains("arr_own(ctx"), "bridge does not make an owned array unique");
    // step returns (array, array, int) as a native tuple the loop destructures
    let step = native_form(&cm, &rs, "step");
    assert!(step.contains("-> (i64, i64, i64)"), "step does not return a native tuple");
    let wl = cm.fns.iter().position(|f| f.name.starts_with("__while")).expect("walk's loop helper");
    let w = rs.find(&format!("fn s_{wl}(")).map(|s| &rs[s..]).expect("walk's loop is native");
    let w = &w[..w[1..].find("\nfn ").map(|e| e + 1).unwrap_or(w.len())];
    assert!(!w.contains("mk_con"), "walk's loop builds heap tuples");
    assert!(w.contains(&format!("= s_{}(ctx, fuel", id("step"))), "the loop does not call step natively");
    // step is a call-free leaf: no fuel settlement of its own; the caller
    // counts its unit in a register
    assert!(!step.contains("*fuel"), "leaf settles fuel through the pointer");
    assert!(w.contains("fl = fl.wrapping_add(1i64);"), "caller does not count the leaf's fuel unit");
    // the if/else accumulator became mask selects
    assert!(rs.contains("m) | (") && rs.contains("m ^ -1i64)"), "if-converted selects are not mask arithmetic");
    // one array lent and moved into the same call: the net inlines the
    // callee, and the native body must read the array before writing it
    // in place (value semantics: the read sees the old element)
    let al = native_form(&cm, &rs, "alias");
    let (rd, wr) = (al.find("arr_get").expect("alias reads"), al.find("arr_set").expect("alias writes"));
    assert!(rd < wr, "alias writes the array before reading it: {al}");
    // a dive-form function returning a tuple hands it back unboxed
    let sp = id("split");
    assert!(rs.contains(&format!("fn n_{sp}(")), "split has no native multi-value entry");
    assert!(dive_form(&cm, &rs, "listy").contains(&format!("n_{sp}(ctx, fuel")) || rs.contains(&format!("n_{sp}(ctx, fuel")), "listy does not take split's components natively");
}


// ---- native int representations (plain / pre-shifted) ----

#[test]
fn int_representations_agree_with_oracle() {
    let src = fixture("int_reps.py");
    let mut m = mithril_front::parse(&src).unwrap_or_else(|d| panic!("parse: line {}: {}", d.line, d.msg));
    let _ = mithril_reassoc::analyze(&mut m);
    let cm = desugar(&m).unwrap_or_else(|d| panic!("desugar: line {}: {}", d.line, d.msg));
    let (sm, _) = mithril_net::specialize(&cm, 1 << 20);
    let want = oracle(&cm);
    let id = |n: &str| cm.fns.iter().position(|f| f.name == n).unwrap();
    for (tag, rep) in [("chosen", None), ("plain", Some(false)), ("shifted", Some(true))] {
        let rs = mithril_codegen::emit_rust_opts(&sm, mithril_codegen::EmitOpts { int_rep: rep });
        // the arithmetic, the DP rows and the recursion all run natively
        for f in ["mix", "dp", "walk"] {
            assert!(rs.contains(&format!("fn s_{}(", id(f))), "{tag}: {f} is not native");
        }
        if rep == Some(true) {
            let native = &rs[rs.find("fn s_").unwrap()..rs.find("fn net_entries").unwrap()];
            assert!(!native.contains("wrap56("), "shifted: native code still re-wraps");
        }
        if rep.is_none() {
            // the rolling-row inner loop keeps array lengths in locals
            assert!(rs.contains("arr_get_n("), "chosen: DP loop does not use length locals");
        }
        let bin = compile(&rs, &format!("int_reps_{tag}"));
        for t in ["1", "4"] {
            assert_eq!(run(&bin, &[t]), want, "int_reps {tag} --threads {t}");
            assert_eq!(run(&bin, &[t, "3"]), want, "int_reps {tag} --threads {t} fuel 3");
        }
    }
}

#[test]
fn raw_int_array_converts_on_first_non_int_write() {
    // all-int arrays store pre-shifted words; storing a list converts the
    // array to tagged elements (and a shared copy keeps its raw words)
    let (_cm, rs) = trmc_golden("hetero_array.py");
    // the write path converts (arr_set and arr_unraw live in mithril_rt::prelude)
    assert!(rs.contains("arr_set(ctx"), "the boxed write path is not used");
    let rt = std::fs::read_to_string(ws_root().join("crates/mithril-rt/src/prelude.rs")).unwrap();
    assert!(rt.contains("arr_unraw(a)"), "the write path does not convert raw words to tagged");
}

#[test]
fn heavy_fold_splits_by_measured_work() {
    let (_cm, rs) = trmc_golden("heavy_fold.py");
    assert!(rs.contains("FOLD_EST_"), "fold has no work estimate");
    assert!(rs.contains("sat_mul(") && rs.contains("est)"), "fold split ignores the estimate");
}

#[test]
fn value_branches_release_what_their_arms_skip() {
    // A leak here grows with the loop: 200k iterations exhaust a small
    // cell arena, and leaked arrays show in the live-array count.
    let (cm, rs) = trmc_golden("value_branch_ownership.py");
    let want = oracle(&cm);
    let bin = compile(&rs, "value_branch_ownership_small_arena");
    for t in ["1", "4"] {
        let (out, err) = run_env(&bin, &[t], &[("MITHRIL_NODES", "65536"), ("MITHRIL_STATS", "1")]);
        assert_eq!(out.trim(), want, "--threads {t} with a small arena");
        assert!(err.contains("arrays_live=0"), "--threads {t}: arrays leaked: {err}");
    }
}

#[test]
fn mutual_tail_recursion_becomes_a_loop() {
    let (cm, rs) = trmc_golden("mutual_tail.py");
    let ev = native_form(&cm, &rs, "ev");
    let id = cm.fns.iter().position(|f| f.name == "od").unwrap();
    assert!(ev.contains("continue 'l"), "ev is not a loop");
    assert!(!ev.contains(&format!("s_{id}(")), "ev still calls od");
}

#[test]
fn f32_primitives_match_oracle_in_every_representation() {
    let src = fixture("f32_ops.py");
    let mut m = mithril_front::parse(&src).unwrap_or_else(|d| panic!("parse: line {}: {}", d.line, d.msg));
    let _ = mithril_reassoc::analyze(&mut m);
    let cm = desugar(&m).unwrap_or_else(|d| panic!("desugar: line {}: {}", d.line, d.msg));
    let (sm, _) = mithril_net::specialize(&cm, 1 << 20);
    let want = oracle(&cm);
    for (tag, rep) in [("chosen", None), ("plain", Some(false)), ("shifted", Some(true))] {
        let rs = mithril_codegen::emit_rust_opts(&sm, mithril_codegen::EmitOpts { int_rep: rep });
        // the arithmetic runs natively on hardware binary32
        let ops = cm.fns.iter().position(|f| f.name == "ops").unwrap();
        assert!(rs.contains(&format!("fn s_{ops}(")), "{tag}: ops is not native");
        assert!(rs.contains("f32_mul("), "{tag}: no hardware f32 multiply");
        let bin = compile(&rs, &format!("f32_ops_{tag}"));
        for t in ["1", "4"] {
            assert_eq!(run(&bin, &[t]), want, "f32_ops {tag} --threads {t}");
        }
    }
}

// ---- closures through the compiled runtime (Phase B) ----

#[test]
fn closures_match_oracle_under_suspension() {
    // every closure shape of the fixture, at 1/4/16 threads and under
    // fuel starvation (suspensions inside and around applications)
    let (_cm, rs) = trmc_golden("closures.py");
    assert!(rs.contains("build_closure(ctx, "), "no closure is built by compiled code");
    assert!(rs.contains("apply(ctx, "), "no closure is applied by compiled code");
    // the rule form applies through the net with a continuation record
    assert!(rs.contains("apply_spawn(ctx, "), "the rule form does not apply closures");
}

#[test]
fn shared_closure_runs_its_free_work_once_at_runtime() {
    // W2: `mk(k) = λx. x + heavy(k)` applied N times. heavy(k) is a
    // compiled call the net runs once and shares; each application costs
    // a constant number of rewrites, so rewrites grow linearly in N with a
    // small slope, while a strict evaluation would run heavy N times.
    let src = fixture("w2_runtime.py");
    let (cm, rs) = pipeline(&src, 0);
    let want = oracle(&cm);
    let bin = compile(&rs, "w2_runtime");
    let rewrites = |stderr: &str| -> u64 {
        stderr
            .split_whitespace()
            .find_map(|w| w.strip_prefix("rewrites="))
            .unwrap_or_else(|| panic!("no rewrites in stderr: {stderr:?}"))
            .parse()
            .unwrap()
    };
    let (out, err) = run_env(&bin, &["1"], &[("MITHRIL_STATS", "1")]);
    assert_eq!(out, want);
    let n = 4096u64;
    let r = rewrites(&err);
    assert!(r < 16 * n, "sharing lost: {r} rewrites for {n} applications");
    // parallel: same value, sharing kept
    let (out16, err16) = run_env(&bin, &["16"], &[("MITHRIL_STATS", "1")]);
    assert_eq!(out16, want);
    assert!(rewrites(&err16) < 16 * n, "sharing lost in parallel: {}", rewrites(&err16));
    // the strict shape as a twin: heavy(k) called per iteration costs one
    // compiled call per application (rewrites >= N), so the runtime work
    // differs by the work of heavy per call
    let strict = src.replace("return loop(g, i - 1, acc + g(i))", "return loop(g, i - 1, acc + i + heavy(array_len(array_new(20, 0))))");
    let (scm, srs) = pipeline(&strict, 0);
    let sbin = compile(&srs, "w2_strict");
    let (sout, serr) = run_env(&sbin, &["16"], &[("MITHRIL_STATS", "1")]);
    assert_eq!(sout, oracle(&scm));
    assert!(rewrites(&serr) >= n, "strict twin did not call heavy per iteration: {}", rewrites(&serr));
}
