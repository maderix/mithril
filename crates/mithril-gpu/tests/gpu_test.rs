//! mithril-gpu tests.
//!
//! Pure emission tests always run. GPU-gated tests are `#[ignore]`d and
//! additionally no-op unless MITHRIL_GPU=1; run them with
//!     MITHRIL_GPU=1 cargo test -p mithril-gpu -- --ignored --test-threads=1
//! (single-threaded because the exhaustion test mutates MITHRIL_GPU_NODES).

use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, CoreFn, CoreModule};
use mithril_gpu::{compile_and_run, emit_cuda, ENGINE_CU};
use mithril_rt::Redex;
use std::path::PathBuf;

fn f(name: &str, arity: usize, body: Core) -> CoreFn {
    CoreFn { name: name.into(), arity, body, self_tail_rec: false, fold: None }
}

fn num(n: i64) -> Core {
    Core::Num(n)
}
fn var(i: u32) -> Core {
    Core::Var(i)
}
fn op(o: BinOp, a: Core, b: Core) -> Core {
    Core::Op2(o, Box::new(a), Box::new(b))
}
fn cmp(o: CmpOp, a: Core, b: Core) -> Core {
    Core::Cmp(o, Box::new(a), Box::new(b))
}
fn iff(c: Core, t: Core, e: Core) -> Core {
    Core::If(Box::new(c), Box::new(t), Box::new(e))
}
fn lt(x: u32, r: Core, b: Core) -> Core {
    Core::Let(x, Box::new(r), Box::new(b))
}
fn call(fid: u32, args: Vec<Core>) -> Core {
    Core::Call(fid, args)
}

fn num_port(v: i64) -> u64 {
    (2u64 << 56) | ((v as u64) & ((1u64 << 56) - 1))
}

/// fib(n) = n if n < 2 else fib(n-1) + fib(n-2); main(n) = fib(n).
fn fib_module() -> CoreModule {
    let fib = iff(
        cmp(CmpOp::Lt, var(0), num(2)),
        var(0),
        op(
            BinOp::Add,
            call(0, vec![op(BinOp::Sub, var(0), num(1))]),
            call(0, vec![op(BinOp::Sub, var(0), num(2))]),
        ),
    );
    CoreModule {
        fns: vec![f("fib", 1, fib), f("main", 1, call(0, vec![var(0)]))],
        ctors: vec![],
        main: 1,
    }
}

/// helper(x) = x*2 + 3; main(x) = helper(x) + 4. Completes inside one dive.
fn trivial_module() -> CoreModule {
    let helper = op(BinOp::Add, op(BinOp::Mul, var(0), num(2)), num(3));
    let main = op(BinOp::Add, call(0, vec![var(0)]), num(4));
    CoreModule { fns: vec![f("helper", 1, helper), f("main", 1, main)], ctors: vec![], main: 1 }
}

/// sum(n) = 0 if n == 0 else (let x = sum(n-1) in x + n): a deep sequential
/// chain that exercises per-program continuations (Let over a call).
fn sum_module() -> CoreModule {
    let body = iff(
        cmp(CmpOp::Eq, var(0), num(0)),
        num(0),
        lt(1, call(0, vec![op(BinOp::Sub, var(0), num(1))]), op(BinOp::Add, var(1), var(0))),
    );
    CoreModule { fns: vec![f("sum", 1, body)], ctors: vec![], main: 0 }
}

/// build(n) = Leaf if n == 0 else Node(build(n-1), build(n-1));
/// tsum(Leaf) = 1, tsum(Node l r) = tsum(l) + tsum(r);
/// main(n) = tsum(build(n)).
fn tree_module() -> CoreModule {
    let build = iff(
        cmp(CmpOp::Eq, var(0), num(0)),
        Core::Ctor(0, vec![]),
        Core::Ctor(
            1,
            vec![
                call(0, vec![op(BinOp::Sub, var(0), num(1))]),
                call(0, vec![op(BinOp::Sub, var(0), num(1))]),
            ],
        ),
    );
    let tsum = Core::Match(
        Box::new(var(0)),
        vec![
            (0, vec![], num(1)),
            (1, vec![1, 2], op(BinOp::Add, call(1, vec![var(1)]), call(1, vec![var(2)]))),
        ],
    );
    let main = lt(1, call(0, vec![var(0)]), call(1, vec![var(1)]));
    CoreModule {
        fns: vec![f("build", 1, build), f("tsum", 1, tsum), f("main", 1, main)],
        ctors: vec![("Leaf".into(), 0), ("Node".into(), 2)],
        main: 2,
    }
}

fn gpu_on() -> bool {
    std::env::var("MITHRIL_GPU").ok().as_deref() == Some("1")
}

fn cache_dir() -> PathBuf {
    let base = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("mithril-gpu-cache")
}

// ---------------- pure emission tests (always run) ----------------

#[test]
fn emit_has_fire_and_dive_arms_for_each_function() {
    let src = emit_cuda(&fib_module());
    assert!(src.contains("__device__ void prog_fire("), "missing prog_fire:\n{src}");
    assert!(src.contains("__device__ u64 prog_dive("), "missing prog_dive");
    for i in 0..2 {
        assert!(src.contains(&format!("u64 dv_{i}(")), "missing dive fn dv_{i}");
        assert!(src.contains(&format!("void fb_{i}(")), "missing fire body fb_{i}");
        assert!(src.contains(&format!("void ent_{i}(")), "missing entry ent_{i}");
        assert!(src.contains(&format!("case {i}u: return dv_{i}(")), "missing dive arm {i}");
    }
    // boot rule 0 routes to main (fn 1), fn entry rules are 1-based
    assert!(src.contains("case 0u: ent_1(e0, e1, e2); return;"));
    assert!(src.contains("case 1u: ent_0(e0, e1, e2); return;"));
    assert!(src.starts_with("// program.cu"));
    assert!(src.contains("#define PROG_NRULES "));
    assert!(src.contains("#include \"engine.cu\""));
    // fib's parallel decomposition uses the generic Op2 join
    assert!(src.contains("d_op2(rr.s, e0, e1)"));
}

#[test]
fn emit_dual_mode_shapes_and_fuel() {
    let src = emit_cuda(&fib_module());
    // dive form: entry fuel check + suspension sentinel
    assert!(src.contains("if (--(*fuel) < 0) return SUSP;"));
    assert!(src.contains("int fu = DIVE_FUEL;"));
    assert!(src.contains("if (r != SUSP) { deliver(e2, r); return; }"));
    // fire form spawns child calls into the callee's bucket
    assert!(src.contains("spawn3(1u,"));
}

#[test]
fn emit_no_rust_syntax() {
    for m in [fib_module(), trivial_module(), sum_module(), tree_module()] {
        let src = emit_cuda(&m);
        for tok in ["fn ", "let ", "pub ", "match ", "&mut", "::", "->"] {
            assert!(!src.contains(tok), "rust token {tok:?} leaked into:\n{src}");
        }
        let open = src.matches('{').count();
        let close = src.matches('}').count();
        assert_eq!(open, close, "unbalanced braces");
    }
}

#[test]
fn emit_is_deterministic() {
    let a = emit_cuda(&tree_module());
    let b = emit_cuda(&tree_module());
    assert_eq!(a, b);
}

#[test]
fn emit_ctor_match_and_continuations() {
    let src = emit_cuda(&tree_module());
    // constructors build cell chains; match dispatches on con_tag
    assert!(src.contains("mk_con("));
    assert!(src.contains("con_tag("));
    assert!(src.contains("alloc_node("));
    // main's `let t = build(n) in tsum(t)` needs a per-program continuation
    assert!(src.contains("kf_"), "expected a continuation rule:\n{src}");
    assert!(src.contains("alloc_rec("));
    assert!(src.contains("deliver("));
}

#[test]
fn engine_source_is_program_independent() {
    assert!(ENGINE_CU.contains("PROG_NRULES"));
    assert!(ENGINE_CU.contains("__global__ void k_fire"));
    assert!(ENGINE_CU.contains("__global__ void k_boot"));
    // the spike leak fix: overflow ring + checked bump + abort flag
    assert!(ENGINE_CU.contains("ovf"));
    assert!(ENGINE_CU.contains("g_abort(AB_ARENA)"));
    assert!(!ENGINE_CU.contains("prog_fire(u32 rule, u64 e0, u64 e1, u64 e2) {"));
}

// ---------------- GPU-gated tests (MITHRIL_GPU=1) ----------------

#[test]
#[ignore = "requires MITHRIL_GPU=1 (4090 + docker blaze-ptx:cu13x)"]
fn gpu_syntax_gate_nvcc_compiles_two_fn_module() {
    if !gpu_on() {
        return;
    }
    let src = emit_cuda(&fib_module());
    let dir = cache_dir().join("syntax-gate");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("program.cu"), &src).unwrap();
    std::fs::write(dir.join("engine.cu"), ENGINE_CU).unwrap();
    let dir = dir.canonicalize().unwrap();
    let out = std::process::Command::new("docker")
        .args([
            "run", "--rm", "--gpus", "all",
            "-v", &format!("{}:/w", dir.display()),
            "blaze-ptx:cu13x",
            "nvcc", "-O3", "-arch=sm_89", "-cubin", "/w/program.cu", "-o", "/w/program.cubin",
        ])
        .output()
        .expect("docker not runnable");
    assert!(
        out.status.success(),
        "nvcc rejected emitted CUDA:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_trivial_dive_only_program_end_to_end() {
    if !gpu_on() {
        return;
    }
    let src = emit_cuda(&trivial_module());
    // boot: rule 0 = main entry, a = arg0, parent = ROOT (0)
    let boot = Redex { a: num_port(20), b: 0, aux: 0 };
    let got = compile_and_run(&src, boot, &cache_dir()).expect("gpu run failed");
    assert_eq!(got, num_port(20 * 2 + 3 + 4), "main(20) = 47");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_fib_20_fire_decomposition_end_to_end() {
    if !gpu_on() {
        return;
    }
    let src = emit_cuda(&fib_module());
    let boot = Redex { a: num_port(20), b: 0, aux: 0 };
    let got = compile_and_run(&src, boot, &cache_dir()).expect("gpu run failed");
    assert_eq!(got, num_port(6765), "fib(20)");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_sum_chain_continuations_end_to_end() {
    if !gpu_on() {
        return;
    }
    let src = emit_cuda(&sum_module());
    let boot = Redex { a: num_port(1000), b: 0, aux: 0 };
    let got = compile_and_run(&src, boot, &cache_dir()).expect("gpu run failed");
    assert_eq!(got, num_port(500500), "sum(1000)");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1"]
fn gpu_tree_sum_ctors_and_match_end_to_end() {
    if !gpu_on() {
        return;
    }
    let src = emit_cuda(&tree_module());
    let boot = Redex { a: num_port(10), b: 0, aux: 0 };
    let got = compile_and_run(&src, boot, &cache_dir()).expect("gpu run failed");
    assert_eq!(got, num_port(1 << 10), "tsum(build(10)) counts 1024 leaves");
}

#[test]
#[ignore = "requires MITHRIL_GPU=1 (mutates MITHRIL_GPU_NODES; run --test-threads=1)"]
fn gpu_arena_exhaustion_is_a_clean_error() {
    if !gpu_on() {
        return;
    }
    let src = emit_cuda(&tree_module());
    let boot = Redex { a: num_port(20), b: 0, aux: 0 }; // 2^20-leaf tree
    std::env::set_var("MITHRIL_GPU_NODES", "1024");
    let got = compile_and_run(&src, boot, &cache_dir());
    std::env::remove_var("MITHRIL_GPU_NODES");
    let err = got.expect_err("a 2^20-leaf tree cannot fit in 1024 cells");
    assert!(err.contains("arena exhausted"), "wrong error: {err}");
    let low = err.to_lowercase();
    assert!(!low.contains("illegal"), "leaked CUDA error text: {err}");
    assert!(!err.contains("CUDA_ERROR"), "leaked CUDA error text: {err}");
}
