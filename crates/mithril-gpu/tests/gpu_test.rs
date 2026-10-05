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
    "big_ints.py",
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
    "fold_borrowed_extra.py",
    "fill_loops.py",
    "grid_fill.py",
    "closure_split.py",
    "loop_split_bounded.py",
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

/// A lowered program: its device artefact, or the constant compile-time
/// reduction left (nothing to run).
enum Prog {
    Cubin(PathBuf),
    Constant(String),
}

/// Lower each source and compute its oracle value, every source on its own
/// thread, then compile the device programs together (one compiler per
/// CPU): what a suite of programs costs on a cold cache is the compiles.
/// Returns (oracle value, program) in order.
fn prepare(srcs: &[String]) -> Vec<(String, Prog)> {
    let lowered: Vec<(String, Result<String, String>)> = std::thread::scope(|s| {
        let hs: Vec<_> = srcs.iter().map(|src| s.spawn(move || {
            let (cm, cu) = pipeline_src(src);
            (oracle(&cm), cu)
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let cus: Vec<String> = lowered.iter().filter_map(|(_, cu)| cu.as_ref().ok().cloned()).collect();
    let mut cubins = mithril_gpu::compile_all(&cus, &cache_dir()).into_iter();
    lowered
        .into_iter()
        .map(|(want, cu)| match cu {
            Ok(_) => (want, Prog::Cubin(cubins.next().unwrap().unwrap_or_else(|e| panic!("{e}")))),
            Err(v) => (want, Prog::Constant(v)),
        })
        .collect()
}

fn run_prog(p: &Prog) -> Result<mithril_gpu::GpuResult, String> {
    match p {
        Prog::Cubin(c) => mithril_gpu::run_cubin(c, BOOT),
        Prog::Constant(v) => Ok(mithril_gpu::GpuResult { port: 0, text: v.clone(), rounds: 0, cell_readback_bytes: 0 }),
    }
}

/// Fixtures prepared together, then run one after another on the device.
fn run_fixtures(names: &[&str]) -> Vec<(String, Result<mithril_gpu::GpuResult, String>)> {
    let srcs: Vec<String> = names.iter().map(|n| fixture(n)).collect();
    prepare(&srcs).into_iter().map(|(want, p)| (want, run_prog(&p))).collect()
}

fn run_fixture(name: &str) -> (String, Result<mithril_gpu::GpuResult, String>) {
    run_fixtures(&[name]).pop().unwrap()
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (4090 + docker mithril-nvcc image)"]
fn gpu_fixtures_match_the_oracle() {
    if !gpu_on() {
        return;
    }
    let mut failed = Vec::new();
    for (name, (want, got)) in FIXTURES.iter().zip(run_fixtures(FIXTURES)) {
        match got {
            Ok(r) if r.text == want => {}
            Ok(r) => failed.push(format!("{name}: device printed {} but the oracle says {want}", r.text)),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    assert!(failed.is_empty(), "GPU results differ from the oracle:\n{}", failed.join("\n"));
}

/// fill_uninit.py's value, computed independently (CPython with every op
/// wrapped to 64 bits; the oracle copies an array per update, too slow at
/// this size).
const FILL_UNINIT: &str = "(3451376496, 1884045312, 7, 200712762, 7, 2389189737, 7, 701548700, 732613360)";

#[test]
fn large_fills_print_as_range_launches() {
    // every proven fill gets the launch; only a fresh array a fill covers in
    // full is allocated without its initial value (full, stencil)
    let (_, cu) = pipeline("fill_uninit.py");
    let cu = cu.expect("fill_uninit runs on the device");
    // full, short, offset, peek and stencil fill; shifted is no fill and
    // total's hash (s * 31 + x) is no proven fold
    assert!(cu.contains("#define RANGE_FILLS 5"));
    assert_eq!(cu.matches("range_launch(").count(), 5);
    assert!(cu.contains("__device__ i64 prog_range_leaf("));
    assert_eq!(cu.matches("arr_new_raw_uninit(").count(), 4, "full and stencil, each in its native and dive forms");
}

#[test]
fn large_sums_print_as_range_launches_from_the_identity() {
    // an int sum's range launch names its kind (1: mod 2^64, 2: mod 2^32) and
    // its leaf starts each term from 0; the device results are checked by
    // gpu_fixtures_match_the_oracle (fold_sum, fold_sum_masked, heavy_fold)
    for (name, kind) in [("fold_sum.py", 1), ("fold_sum_masked.py", 2)] {
        let (_, cu) = pipeline(name);
        let cu = cu.expect("runs on the device");
        let launch = cu.split("range_launch(").nth(1).expect("a range launch");
        let args = &launch[..launch.find(");").unwrap()];
        assert!(args.ends_with(&format!(", {kind}u")), "{name}: {args}");
        let leaf = &cu[cu.find("prog_range_leaf(u32 fid").unwrap()..];
        assert!(leaf.contains("(&fuel, i, i + 1, 0"), "{name}: the term does not start from the identity");
    }
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_range_launched_fills_match_independent_values() {
    if !gpu_on() {
        return;
    }
    let (_, cu) = pipeline("fill_uninit.py");
    let r = compile_and_run(&cu.unwrap(), BOOT, &cache_dir()).unwrap();
    assert_eq!(r.text, FILL_UNINIT);
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_frame_split_shared_environments_match_oracle() {
    if !gpu_on() { return; }
    let (want, got) = run_fixture("fork_split_shapes.py");
    assert_eq!(got.map(|r| r.text), Ok(want));
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
    let corpus = [("stage_closure.py", 50), ("pipeline_cfg.py", 20), ("interp_closure.py", 200)];
    let srcs: Vec<String> = corpus
        .iter()
        .map(|(name, small)| {
            let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/general").join(name);
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e));
            let at = text.rfind("return run(").expect("main returns run(N)");
            let end = at + text[at..].find(')').unwrap();
            format!("{}return run({small}{}", &text[..at], &text[end..])
        })
        .collect();
    for ((name, _), (want, prog)) in corpus.iter().zip(prepare(&srcs)) {
        match run_prog(&prog) {
            Ok(r) if r.text == want => {}
            Ok(r) => failed.push(format!("{name}: device printed {} but the oracle says {want}", r.text)),
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    assert!(failed.is_empty(), "GPU results differ from the oracle:\n{}", failed.join("\n"));
}

/// A recursive tree exposes work through fork levels, with oracle-equal results.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_schedule_is_bounded_by_fork_levels() {
    if !gpu_on() { return; }
    let (want, got) = run_fixture("recursive_counts.py");
    let r = got.expect("device run");
    assert_eq!(r.text, want);
    assert!(r.rounds < 400, "the schedule took {} rounds", r.rounds);
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

/// The black hole demo at a few pixels: its ray march recurses through a
/// join point (march -> __join -> march, an if with code after it). In the
/// device's frame machine those are tail calls: no frame or record is
/// pushed for them, where each step used to push one to global memory.
fn black_hole_small() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/black_hole.py");
    let mut src = std::fs::read_to_string(&p).unwrap();
    for (f, v) in [("width", "8"), ("height", "6"), ("frames", "1"), ("first_frame", "300")] {
        let head = format!("def {f}():\n    return ");
        let at = src.find(&head).unwrap_or_else(|| panic!("{f}() not in the demo")) + head.len();
        let end = at + src[at..].find('\n').unwrap();
        src.replace_range(at..end, v);
    }
    src
}

#[test]
fn march_through_a_join_point_reserves_no_frames() {
    let (cm, cu) = pipeline_src(&black_hole_small());
    let cu = cu.expect("not a constant");
    let march = cm.fns.iter().position(|f| f.name == "march").expect("march");
    let head = format!("native_s_{march}(i64* fuel, u64 pc");
    let def = cu.match_indices(&head).map(|(i, _)| i).find(|&i| cu[i..].split('\n').next().unwrap().ends_with('{')).unwrap_or_else(|| panic!("march has no frame machine"));
    // generated code is not indented: the machine ends where the next definition starts
    let end = ["\n__device__", "\nextern"].iter().filter_map(|h| cu[def..].find(h)).min().unwrap();
    let body = &cu[def..def + end];
    for push in ["native_reserve(", "native_record_push("] {
        assert!(!body.contains(push), "march's machine pushes a frame per step ({push})");
    }
}

/// The same small black hole on the device equals the oracle.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_black_hole_small_matches_the_oracle() {
    if !gpu_on() { return; }
    let (cm, cu) = pipeline_src(&black_hole_small());
    let got = compile_and_run(&cu.expect("not a constant"), BOOT, &cache_dir());
    assert_eq!(got.map(|r| r.text), Ok(oracle(&cm)));
}

/// Defunctionalized native recursion spills into the cell arena while the
/// hardware stack stays fixed. The independent iterative oracle avoids
/// depending on the reference interpreter's own recursion limit.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_deep_native_frames_spill_with_a_fixed_hardware_stack() {
    if !gpu_on() { return; }
    let src = "def count(n):\n    if n == 0:\n        return 0\n    return (count(n - 1) * 3 + 1) & 4294967295\n\ndef main():\n    return count(array_len(array_new(100000, 0)))\n";
    let (_, cu) = pipeline_src(src);
    let cu = cu.expect("not a constant");
    let want = (0..100000).fold(0u32, |n, _| n.wrapping_mul(3).wrapping_add(1)).to_string();
    std::env::set_var("MITHRIL_GPU_STACK", "8192");
    for cache in ["0", "auto"] {
        std::env::set_var("MITHRIL_GPU_NATIVE_WORDS", cache);
        let got = compile_and_run(&cu, BOOT, &cache_dir());
        assert_eq!(got.map(|r| r.text), Ok(want.clone()), "cache={cache}");
    }
    std::env::remove_var("MITHRIL_GPU_NATIVE_WORDS");
    std::env::remove_var("MITHRIL_GPU_STACK");
}

/// A boxed recursion remains in the rule engine. Under a dive budget of
/// 1000 its 600 calls still exercise hardware-stack growth and the guard.
const DEEP600: &str = "@data\nclass Chain:\n    End: ()\n    Node: (tail,)\n\ndef count(n):\n    if n == 0:\n        return End()\n    return Node(count(n - 1))\n\ndef depth(x):\n    match x:\n        case End():\n            return 0\n        case Node(t):\n            return depth(t) + 1\n\ndef main():\n    return depth(count(array_len(array_new(600, 0))))\n";

fn cubin_of(src: &str) -> (String, PathBuf) {
    cubins_of(&[src.to_string()]).pop().unwrap()
}

/// Device programs of `srcs`, prepared together (see `prepare`).
fn cubins_of(srcs: &[String]) -> Vec<(String, PathBuf)> {
    prepare(srcs)
        .into_iter()
        .map(|(want, p)| match p {
            Prog::Cubin(c) => (want, c),
            Prog::Constant(v) => panic!("not a device program: the constant {v}"),
        })
        .collect()
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
    let got = run_fixtures(&["f32_surface.py", "tree_sum.py", "arrays.py"]);
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

/// A small structure around a large array (an image as `(w, h, pixels)`)
/// reads only its own cells, not a snapshot of every cell the run allocated.
/// A result with many cells switches to the snapshot and prints the same.
#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_readback_of_a_small_structure_reads_only_its_cells() {
    if !gpu_on() { return; }
    let src = "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\n\
def build(k, acc):\n    if k == 0:\n        return acc\n    return build(k - 1, Cons(k, acc))\n\n\
def count(l):\n    match l:\n        case Nil():\n            return 0\n        case Cons(h, t):\n            return 1 + count(t)\n\n\
def main():\n    n = array_len(array_new(20000, 0))\n    c = count(build(n, Nil()))\n    return (c, n, array_new(64, c))\n";
    let (want, cubin) = cubin_of(src);
    let r = mithril_gpu::run_cubin(&cubin, BOOT).expect("device run");
    assert_eq!(r.text, want);
    assert_eq!(r.cell_readback_bytes, 32, "a 3-tuple is two cells, whatever the run allocated");
    let many = "def main():\n    n = array_len(array_new(300, 0))\n    return array_new(n, (n, n + 1))\n";
    let (want, cubin) = cubin_of(many);
    let r = mithril_gpu::run_cubin(&cubin, BOOT).expect("device run");
    assert_eq!(r.text, want, "past the single-read limit the snapshot prints the same value");
    assert!(r.cell_readback_bytes > 16 * 64, "300 tuples exceed the single-read limit");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_readback_follows_value_storage_including_array_elements() {
    if !gpu_on() { return; }
    let cases = [
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
    ];
    let srcs: Vec<String> = cases.iter().map(|(body, _)| format!("@data\nclass Box:\n    Empty: ()\n    Wrap: (v,)\n\ndef main():\n    n = array_len(array_new(3, 0))\n    {body}\n")).collect();
    for ((body, needs_cells), (want, cubin)) in cases.into_iter().zip(cubins_of(&srcs)) {
        let r = mithril_gpu::run_cubin(&cubin, BOOT).expect(body);
        assert_eq!(r.text, want, "{body}");
        assert_eq!(r.cell_readback_bytes != 0, needs_cells, "{body}: cell storage demand");
    }
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_BUCKET; run --test-threads=1)"]
fn gpu_array_erasure_does_not_expand_a_suffix_into_a_frontier() {
    if !gpu_on() { return; }
    let srcs: Vec<String> = [0, 1, 255, 256, 257, 1024].into_iter().map(|n| fixture("array_erase_frontier.py").replace("array_new(1024, 0)", &format!("array_new({n}, 0)"))).collect();
    let programs = cubins_of(&srcs);
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

#[test]
#[ignore = "requires MITHRIL_GPU=1 (sets MITHRIL_GPU_NODES and _FUEL; run --test-threads=1)"]
fn gpu_session_runs_match_standalone_runs() {
    if !gpu_on() {
        return;
    }
    let before = {
        let _hold = mithril_gpu::hold_context().unwrap();
        let (want, got) = run_fixture("fib_naive.py");
        assert_eq!(got.map(|r| r.text), Ok(want));
        mithril_gpu::free_vram().unwrap()
    };
    let programs = cubins_of(&["fib_naive.py", "tree_sum.py", "array_erase_frontier.py", "readback_values.py"].map(fixture));
    // two programs with one layout: each takes over the other's dirty buffers
    let fib = |n: u32| format!("def fib(n):\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n\ndef main():\n    return fib(array_len(array_new({n}, 0)))\n");
    let twins = cubins_of(&[fib(18), fib(21)]);
    let session = mithril_gpu::GpuSession::open().unwrap();
    assert!(mithril_gpu::GpuSession::open().is_err(), "a second session opened");
    for k in [0, 1, 0, 1, 1, 0] {
        let (want, cubin) = &twins[k];
        assert_eq!(session.run(cubin, BOOT, None).map(|r| r.text).as_ref(), Ok(want), "twin {k}");
    }
    // the same program again (buffers and module reused), other programs
    // (another layout frees and resizes), a failed run, and a run whose
    // stack doubles: each gives the standalone answer
    for k in [0, 0, 1, 0, 2, 2, 3, 1, 0] {
        let (want, cubin) = &programs[k];
        assert_eq!(session.run(cubin, BOOT, None).map(|r| r.text).as_ref(), Ok(want), "program {k}");
    }
    std::env::set_var("MITHRIL_GPU_NODES", "1024");
    assert!(session.run(&programs[1].1, BOOT, None).is_err(), "the tree cannot fit in 1024 cells");
    std::env::remove_var("MITHRIL_GPU_NODES");
    for k in [1, 0] {
        let (want, cubin) = &programs[k];
        assert_eq!(session.run(cubin, BOOT, None).map(|r| r.text).as_ref(), Ok(want), "program {k} after a failure");
    }
    std::env::set_var("MITHRIL_GPU_FUEL", "1000");
    let (want, deep) = cubin_of(DEEP600);
    let dir = std::env::temp_dir().join(format!("mithril-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fresh = dir.join("deep.cubin");
    std::fs::copy(&deep, &fresh).unwrap(); // no stack hint yet: it doubles
    assert_eq!(session.run(&fresh, BOOT, None).map(|r| r.text), Ok(want));
    std::env::remove_var("MITHRIL_GPU_FUEL");
    let (want, cubin) = &programs[2];
    assert_eq!(session.run(cubin, BOOT, None).map(|r| r.text).as_ref(), Ok(want), "after the doubling");
    drop(session);
    let _ = std::fs::remove_dir_all(&dir);
    // closing the session returns what it kept, and another can open
    let _hold = mithril_gpu::hold_context().unwrap();
    let mut lost = usize::MAX;
    for _ in 0..10 {
        lost = lost.min(before.saturating_sub(mithril_gpu::free_vram().unwrap()));
        if lost < 64 << 20 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(lost < 64 << 20, "{} MiB of device memory not returned by the session", lost >> 20);
    let again = mithril_gpu::GpuSession::open().unwrap();
    let (want, cubin) = &programs[0];
    assert_eq!(again.run(cubin, BOOT, None).map(|r| r.text).as_ref(), Ok(want));
}
