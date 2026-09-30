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
    "array_erase_frontier.py",
    "array_erase_depth.py",
    "readback_values.py",
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
    "f32_surface.py",
    "tuple_whole.py",
    "tuple_nested.py",
    "tuple_leaves.py",
    "mixed_params.py",
    "stale_ret.py",
    "closure_result.py",
    "closure_parts.py",
    "self_types.py",
    "net_values.py",
    "net_lists.py",
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
    for h in ["alloc2(", "alloc_rec(", "deliver(", "dive_to(", "dive_res(", "dive_res_fork(", "tail_to(", "fork_fuel(", "stack_deep(", "work_fuel(", "pop_chain(", "rec_parent(", "spawn_call(", "mk_con2(", "consume2k(", "dup_val(", "free_val(", "take_field(", "arr_set_n(", "hole_link(", "tup_add("] {
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
        Err(v) => (want, Ok(mithril_gpu::GpuResult { port: 0, text: v, rounds: 0, cell_readback_bytes: 0 })),
    }
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (4090 + docker mithril-nvcc image)"]
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
    // the stack doubled past its 8 KiB start before the device refused
    let bytes: usize = err.split('(').nth(1).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(bytes > 8192, "no doubling before the depth error: {err}");
}

/// A recursion 600 deep under a dive budget of 1000 needs more than the
/// 8 KiB starting stack.
const DEEP600: &str = "def count(n):\n    if n == 0:\n        return 0\n    return (count(n - 1) * 3 + 1) & 4294967295\n\ndef main():\n    return count(array_len(array_new(600, 0)))\n";

fn cubin_of(src: &str) -> (String, PathBuf) {
    let (cm, cu) = pipeline_src(src);
    (oracle(&cm), mithril_gpu::compile_to_cubin(&cu.expect("not a constant"), &cache_dir()).unwrap())
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_FUEL; run --test-threads=1)"]
fn gpu_stack_doubles_until_the_program_fits() {
    if !gpu_on() {
        return;
    }
    std::env::set_var("MITHRIL_GPU_FUEL", "1000");
    let (want, cubin) = cubin_of(DEEP600);
    let bytes = std::fs::read(&cubin).unwrap();
    let (r, used) = mithril_gpu::GpuRunner::run_with_stack(&bytes, BOOT, None);
    // started where the first run ended: no doubling
    let (r2, used2) = mithril_gpu::GpuRunner::run_with_stack(&bytes, BOOT, Some(used));
    std::env::remove_var("MITHRIL_GPU_FUEL");
    assert_eq!(r.map(|r| r.text), Ok(want.clone()), "the doubled run must match the oracle");
    assert!(used >= 16384, "the stack never doubled ({used} bytes)");
    assert_eq!(r2.map(|r| r.text), Ok(want));
    assert_eq!(used2, used, "a run started at the known size doubled again");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_FUEL and _STACK; run --test-threads=1)"]
fn gpu_stack_hint_is_kept_beside_the_artefact() {
    if !gpu_on() {
        return;
    }
    std::env::set_var("MITHRIL_GPU_FUEL", "1000");
    let (want, cubin) = cubin_of(DEEP600);
    let dir = std::env::temp_dir().join(format!("mithril-hint-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // an artefact named like a hint: the hint must not overwrite it
    let art = dir.join("p.stack");
    std::fs::copy(&cubin, &art).unwrap();
    let r = mithril_gpu::run_cubin(&art, BOOT);
    let hint = std::fs::read_to_string(dir.join("p.stack.stack")).unwrap_or_default();
    // a size the user fixed is not written
    std::env::set_var("MITHRIL_GPU_STACK", "65536");
    let fixed = dir.join("q.cubin");
    std::fs::copy(&cubin, &fixed).unwrap();
    let r2 = mithril_gpu::run_cubin(&fixed, BOOT);
    std::env::remove_var("MITHRIL_GPU_STACK");
    std::env::remove_var("MITHRIL_GPU_FUEL");
    assert_eq!(r.map(|r| r.text), Ok(want.clone()));
    assert_eq!(std::fs::read(&art).unwrap(), std::fs::read(&cubin).unwrap(), "the artefact was overwritten");
    assert!(hint.trim().parse::<usize>().is_ok_and(|n| n >= 16384), "hint: {hint:?}");
    assert_eq!(r2.map(|r| r.text), Ok(want));
    assert!(!dir.join("q.cubin.stack").exists(), "a fixed stack size was kept as the program's");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_NODES; run --test-threads=1)"]
fn gpu_failed_runs_leave_no_memory_behind() {
    if !gpu_on() {
        return;
    }
    // a retain held across the runs: a run's release then does not destroy
    // the context, so memory a run fails to free shows
    let _hold = mithril_gpu::hold_context().unwrap();
    // one run first: the context then holds what every run shares (the
    // module's local-memory reserve for the stack)
    let (want, got) = run_fixture("fib_naive.py");
    assert_eq!(got.map(|r| r.text), Ok(want));
    let before = mithril_gpu::free_vram().unwrap();
    std::env::set_var("MITHRIL_GPU_NODES", "1024");
    for _ in 0..3 {
        assert!(run_fixture("tree_sum.py").1.is_err(), "the tree cannot fit in 1024 cells");
    }
    std::env::remove_var("MITHRIL_GPU_NODES");
    for _ in 0..3 {
        let (want, got) = run_fixture("fib_naive.py");
        assert_eq!(got.map(|r| r.text), Ok(want));
    }
    // free memory is a device-wide figure other processes move (a
    // desktop shares the device); a leak persists, noise does not. Most
    // arenas are managed and committed on touch, so this sees buffers
    // committed at allocation and context/module memory; under
    // MITHRIL_GPU_EAGER=1 it sees every arena.
    let mut lost = usize::MAX;
    for _ in 0..10 {
        lost = lost.min(before.saturating_sub(mithril_gpu::free_vram().unwrap()));
        if lost < 64 << 20 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(lost < 64 << 20, "{} MiB of device memory not returned", lost >> 20);
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_EAGER; run --test-threads=1)"]
fn gpu_eager_commit_gives_the_same_results() {
    if !gpu_on() {
        return;
    }
    std::env::set_var("MITHRIL_GPU_EAGER", "1");
    let got: Vec<_> = ["f32_surface.py", "tree_sum.py", "arrays.py"].iter().map(|n| run_fixture(n)).collect();
    std::env::remove_var("MITHRIL_GPU_EAGER");
    for (want, got) in got {
        assert_eq!(got.map(|r| r.text), Ok(want));
    }
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

#[test]
#[ignore = "requires MITHRIL_GPU=1 (runs a child process with MITHRIL_GPU_TIMEOUT)"]
fn gpu_timeout_abandons_a_runaway_loop_and_other_processes_run() {
    if !gpu_on() {
        return;
    }
    // A native loop far longer than the deadline checks nothing per
    // iteration, so it is abandoned at twice the deadline until its process
    // exits: run it in a child process (this test binary, the child test
    // below), then run on the device here.
    let t = std::time::Instant::now();
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--ignored", "--exact", "gpu_timeout_child", "--nocapture", "--test-threads=1"]);
    // a clean device environment: only the switch and this test's deadline
    for (k, _) in std::env::vars().filter(|(k, _)| k.starts_with("MITHRIL_GPU_") && k != "MITHRIL_GPU") {
        cmd.env_remove(k);
    }
    let mut child = cmd
        .env("MITHRIL_GPU_TIMEOUT_CHILD", "1")
        .env("MITHRIL_GPU_TIMEOUT", "2")
        .env("MITHRIL_GPU_STATS", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // the child is bounded too: abandoned at 4 s, it must exit well within 60
    while child.try_wait().unwrap().is_none() {
        if t.elapsed().as_secs() > 60 {
            let _ = child.kill();
            panic!("the child did not exit within 60 s");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let out = child.wait_with_output().unwrap();
    let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "child failed:\n{log}");
    assert!(log.contains("exceeded 2 s") && log.contains("abandoned until the process exits"), "wrong error:\n{log}");
    assert!(t.elapsed().as_secs() < 30, "the child waited for the kernel ({:?})", t.elapsed());
    let (want, got) = run_fixture("fib_naive.py");
    assert_eq!(got.map(|r| r.text), Ok(want), "a run after the child's timeout");
}

/// The child of the test above (does nothing unless asked by it).
#[test]
#[ignore = "run by gpu_timeout_abandons_a_runaway_loop_and_other_processes_run"]
fn gpu_timeout_child() {
    if std::env::var_os("MITHRIL_GPU_TIMEOUT_CHILD").is_none() || !gpu_on() {
        return;
    }
    let src = "def spin(n):\n    s = 0\n    for i in range(n):\n        s = (s * 31 + i) & 4294967295\n    return s\n\ndef main():\n    return spin(array_len(array_new(1, 0)) << 40)\n";
    let (_, cu) = pipeline_src(src);
    let err = compile_and_run(&cu.expect("not a constant"), BOOT, &cache_dir()).expect_err("the run cannot finish in 2 s");
    println!("child error: {err}");
}

/// Dead intermediate cells must not be transferred for an immediate result.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_scalar_readback_does_not_copy_dead_cells() {
    if !gpu_on() { return; }
    let (want, got) = run_fixture("tree_sum.py");
    let r = got.expect("device run");
    assert_eq!(r.text, want);
    assert!(r.rounds > 0, "exercise the device, not a constant result");
    assert_eq!(r.cell_readback_bytes, 0, "the scalar result has no cell references");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_readback_follows_value_storage_including_array_elements() {
    if !gpu_on() { return; }
    for (body, needs_cells) in [
        ("return n", false),
        ("return lambda x: x + n", false),
        ("if n == 3:\n        return Empty()\n    return Wrap(n)", false),
        ("return Wrap(n)", false),
        ("return array_new(n, n)", false),
        ("return array_new(n, array_new(n, n))", false),
        ("return array_get(array_new(n, 2.5), 0)", true),
        ("return array_new(n, 2.5)", true),
        ("return (n, n)", true),
        ("return array_new(n, (n, n))", true),
    ] {
        let src = format!("@data\nclass Box:\n    Empty: ()\n    Wrap: (v,)\n\ndef main():\n    n = array_len(array_new(3, 0))\n    {body}\n");
        let (want, cubin) = cubin_of(&src);
        let r = mithril_gpu::run_cubin(&cubin, BOOT).expect(body);
        assert_eq!(r.text, want, "{body}");
        assert_eq!(r.cell_readback_bytes != 0, needs_cells, "{body}: cell storage demand");
    }
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_BUCKET; run --test-threads=1)"]
fn gpu_array_erasure_does_not_expand_a_suffix_into_a_frontier() {
    if !gpu_on() { return; }
    let programs: Vec<_> = [0, 1, 255, 256, 257, 1024].into_iter().map(|n| {
        cubin_of(&fixture("array_erase_frontier.py").replace("array_new(1024, 0)", &format!("array_new({n}, 0)")))
    }).collect();
    std::env::set_var("MITHRIL_GPU_BUCKET", "64");
    let results: Vec<_> = programs.iter().map(|(_, p)| mithril_gpu::run_cubin(p, BOOT)).collect();
    std::env::remove_var("MITHRIL_GPU_BUCKET");
    for ((want, _), got) in programs.iter().zip(results) {
        assert_eq!(got.map(|r| r.text), Ok(want.clone()), "bounded erasure must preserve shared elements");
    }
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_deep_array_erasure_resumes_after_the_depth_bound() {
    if !gpu_on() { return; }
    let (want, got) = run_fixture("array_erase_depth.py");
    let r = got.expect("deep array erasure");
    assert_eq!(r.text, want);
    assert!(r.rounds > 1, "the remaining suffix must resume as scheduled work");
}
