//! mithril-gpu tests: the CUDA printer of the lowered IR, and the device
//! runtime, against the same fixtures and the same oracle as the CPU
//! backend (crates/mithril-codegen/tests/fixtures).
//!
//! Pure emission tests always run. GPU-gated tests are `#[ignore]`d and
//! additionally no-op unless MITHRIL_GPU=1; run them with
//!     MITHRIL_GPU=1 cargo test -p mithril-gpu --release -- --ignored --test-threads=1
//! (single-threaded: runs share the device, and some tests set capacities
//! through the environment).

use mithril_codegen::fmt_val;
use mithril_front::core::{eval_core, CoreModule};
use mithril_gpu::{compile_and_run, emit_cuda, ENGINE_CU};
use mithril_rt::Redex;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mithril-codegen/tests/fixtures").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e))
}

/// parse -> analyze -> desugar -> specialize (by the net rules) -> emit_cuda.
fn pipeline(name: &str) -> (CoreModule, Result<String, String>) {
    pipeline_src(&fixture(name))
}

fn pipeline_src(src: &str) -> (CoreModule, Result<String, String>) {
    let src = src.to_string();
    // the specializer recurses per net depth: a deep stack, as the CLI has
    std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            let mut m = mithril_front::parse(&src).unwrap_or_else(|d| panic!("parse: line {}: {}", d.line, d.msg));
            let _ = mithril_reassoc::analyze(&mut m);
            let cm = mithril_front::desugar(&m).unwrap_or_else(|d| panic!("desugar: line {}: {}", d.line, d.msg));
            let (sm, _) = mithril_net::specialize(&cm, 1 << 20);
            let cu = emit_cuda(&sm);
            (cm, cu)
        })
        .unwrap()
        .join()
        .unwrap()
}

fn oracle(cm: &CoreModule) -> String {
    let cm = cm.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || fmt_val(&eval_core(&cm, cm.main, &[])))
        .unwrap()
        .join()
        .unwrap()
}

fn gpu_on() -> bool {
    std::env::var("MITHRIL_GPU").ok().as_deref() == Some("1")
}

fn cache_dir() -> PathBuf {
    let base = std::env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("mithril-gpu-cache")
}

/// `main()` takes no arguments: the boot redex carries nothing.
const BOOT: Redex = Redex { a: 0, b: 0, aux: 0 };

/// Every fixture the CPU backend is tested on, closures included.
const FIXTURES: &[&str] = &[
    "fact_while.py",
    "tree_sum.py",
    "fib_naive.py",
    "fold_sum.py",
    "fold_sum_masked.py",
    "wide_loop.py",
    "trmc_list.py",
    "trmc_tree.py",
    "borrow_eval.py",
    "value_branch_ownership.py",
    "table_loop.py",
    "mutual_tail.py",
    "fast_leaf.py",
    "unfold_wrap.py",
    "ifconv_native.py",
    "int_reps.py",
    "native_arrays.py",
    "arrays.py",
    "hetero_array.py",
    "f32_ops.py",
    "heavy_fold.py",
    "fork_reach.py",
    "fork_chain.py",
    "fork_split_shapes.py",
    "chain_boxed.py",
    "deep_leaves.py",
    // the net region: closures built, shared and applied by compiled code
    "closures.py",
    "w2_runtime.py",
];

// ---------------- pure emission tests (always run) ----------------

#[test]
fn every_fixture_prints_as_cuda_with_the_rule_table() {
    for name in FIXTURES {
        let (cm, cu) = pipeline(name);
        let cu = match cu {
            Ok(cu) => cu,
            // the net reduced the whole program at compile time
            Err(v) => {
                assert_eq!(v, oracle(&cm), "{name}: constant differs from the oracle");
                continue;
            }
        };
        assert!(cu.contains("#include \"engine.cu\""), "{name}: no engine include");
        assert!(cu.contains("__device__ R prog_dive("), "{name}: no dive dispatcher");
        assert!(cu.contains("__device__ void prog_fire("), "{name}: no fire dispatcher");
        assert!(cu.contains("case 0u: fc_"), "{name}: no boot rule");
        for tok in ["let ", "&mut", "match ", "Ok(", "Err(", "::<", "u16_(", "wrapping_"] {
            assert!(!cu.contains(tok), "{name}: Rust spelling `{tok}` leaked into CUDA:\n{cu}");
        }
    }
}

#[test]
fn emission_is_deterministic() {
    let (_, a) = pipeline("tree_sum.py");
    let (_, b) = pipeline("tree_sum.py");
    assert_eq!(a, b);
}

#[test]
fn engine_source_is_program_independent() {
    assert!(ENGINE_CU.contains("PROG_NRULES"));
    assert!(ENGINE_CU.contains("__global__ void k_run"), "the driver on the device");
    assert!(ENGINE_CU.contains("__global__ void k_boot"));
    assert!(ENGINE_CU.contains("g_abort(AB_ARENA)"));
    // the program supplies these; the engine only declares them
    assert!(!ENGINE_CU.contains("prog_fire(u32 rule, u64 e0, u64 e1, u64 e2) {"));
    assert!(!ENGINE_CU.contains("bool lin(u16 k) {"));
    // the IR's context vocabulary, on the device without a context object
    for h in ["alloc2(", "alloc_rec(", "deliver(", "dive_to(", "dive_res(", "dive_res_fork(", "tail_to(", "fork_fuel(", "stack_deep(", "pop_chain(", "rec_parent(", "spawn_call(", "mk_con2(", "consume2k(", "dup_val(", "free_val(", "take_field(", "arr_set_n(", "hole_link(", "tup_add("] {
        assert!(ENGINE_CU.contains(h), "engine lacks {h}");
    }
}

// ---------------- GPU-gated tests (MITHRIL_GPU=1) ----------------

fn run_fixture(name: &str) -> (String, Result<mithril_gpu::GpuResult, String>) {
    let (cm, cu) = pipeline(name);
    let want = oracle(&cm);
    match cu {
        Ok(cu) => (want.clone(), compile_and_run(&cu, BOOT, &cache_dir())),
        // nothing to run: the constant is the result
        Err(v) => (want, Ok(mithril_gpu::GpuResult { port: 0, text: v, rounds: 0 })),
    }
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (4090 + docker blaze-ptx:cu13x)"]
fn gpu_fixtures_match_the_oracle() {
    if !gpu_on() {
        return;
    }
    let mut failed = Vec::new();
    for name in FIXTURES {
        let (want, got) = run_fixture(name);
        match got {
            Ok(r) if r.text == want => {}
            Ok(r) => failed.push(format!("{name}: device printed {} but the oracle says {want}", r.text)),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    assert!(failed.is_empty(), "GPU results differ from the oracle:\n{}", failed.join("\n"));
}

/// The closure corpus (bench/general), at the sizes run.py uses for its
/// small lane; the checksum is what the CPU program prints.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_closure_corpus_matches_the_cpu() {
    if !gpu_on() {
        return;
    }
    let mut failed = Vec::new();
    // run.py's small sizes: `return run(N)` in main
    for (name, small) in [("stage_closure.py", 50), ("pipeline_cfg.py", 20), ("interp_closure.py", 200)] {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/general").join(name);
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e));
        let at = text.rfind("return run(").expect("main returns run(N)");
        let end = at + text[at..].find(')').unwrap();
        let src = format!("{}return run({small}{}", &text[..at], &text[end..]);
        let (cm, cu) = pipeline_src(&src);
        let want = oracle(&cm);
        match cu {
            Err(v) => {
                if v != want {
                    failed.push(format!("{name}: constant {v} differs from the oracle {want}"));
                }
            }
            Ok(cu) => match compile_and_run(&cu, BOOT, &cache_dir()) {
                Ok(r) if r.text == want => {}
                Ok(r) => failed.push(format!("{name}: device printed {} but the oracle says {want}", r.text)),
                Err(e) => failed.push(format!("{name}: {e}")),
            },
        }
    }
    assert!(failed.is_empty(), "GPU results differ from the oracle:\n{}", failed.join("\n"));
}

/// The device driver's schedule is bounded by the program's fork levels,
/// not its size: tree-bitonic at depth 8 (256 leaves) is ~200 rounds (one
/// per call level of the merge network; the host wave loop it replaced
/// took thousands, one per rewrite level of a lane's chain).
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_schedule_is_bounded_by_fork_levels() {
    if !gpu_on() {
        return;
    }
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/ports/tree-bitonic.py");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e));
    assert!(text.contains("bsort(23, 0, 0)"), "the port's main changed");
    let src = text.replace("bsort(23, 0, 0)", "bsort(8, 0, 0)");
    let (cm, cu) = pipeline_src(&src);
    let want = oracle(&cm);
    let cu = cu.expect("bitonic is not a compile-time constant");
    let r = compile_and_run(&cu, BOOT, &cache_dir()).expect("device run");
    assert_eq!(r.text, want, "tree-bitonic depth 8 on the device");
    assert!(r.rounds < 400, "the schedule took {} rounds for 8 fork levels", r.rounds);
}

/// A sequential chain costs the device a round per budget of steps, not
/// a round per step: 100,000 dependent steps in under 2,000 rounds (a
/// cut runs inline with the budget in the parallel world; every call a
/// task took 100,001 rounds).
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_chain_costs_a_round_per_budget_not_per_step() {
    if !gpu_on() {
        return;
    }
    let (want, got) = run_fixture("chain_boxed.py");
    let r = got.expect("device run");
    assert_eq!(r.text, want, "chain_boxed on the device");
    assert!(r.rounds < 2000, "the chain took {} rounds", r.rounds);
}

/// A native (scalar) non-tail recursion has no budget; past the thread's
/// stack the guard aborts with a named error instead of a driver fault.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_deep_native_recursion_is_a_clean_error() {
    if !gpu_on() {
        return;
    }
    let src = "def count(n):\n    if n == 0:\n        return 0\n    return (count(n - 1) * 3 + 1) & 4294967295\n\ndef main():\n    return count(array_len(array_new(100000, 0)))\n";
    let (_, cu) = pipeline_src(src);
    let err = compile_and_run(&cu.expect("not a constant"), BOOT, &cache_dir()).expect_err("100,000 native frames cannot fit the device stack");
    assert!(err.contains("recursion too deep"), "wrong error: {err}");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_NODES; run --test-threads=1)"]
fn gpu_oversized_nodes_request_is_capped_not_oom() {
    if !gpu_on() {
        return;
    }
    // 2^30 cells = 16 GiB of nodes: unallocatable next to the other buffers
    // on a 24 GB card. The runner must cap to free VRAM and still run.
    std::env::set_var("MITHRIL_GPU_NODES", "1073741824");
    let (want, got) = run_fixture("fib_naive.py");
    std::env::remove_var("MITHRIL_GPU_NODES");
    assert_eq!(got.map(|r| r.text), Ok(want), "capped arena must still produce the result");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_NODES; run --test-threads=1)"]
fn gpu_arena_exhaustion_is_a_clean_error() {
    if !gpu_on() {
        return;
    }
    std::env::set_var("MITHRIL_GPU_NODES", "1024");
    let (_, got) = run_fixture("tree_sum.py");
    std::env::remove_var("MITHRIL_GPU_NODES");
    let err = got.expect_err("the tree cannot fit in 1024 cells");
    assert!(err.contains("arena exhausted"), "wrong error: {err}");
    let low = err.to_lowercase();
    assert!(!low.contains("illegal"), "leaked CUDA error text: {err}");
    assert!(!err.contains("CUDA_ERROR"), "leaked CUDA error text: {err}");
}
