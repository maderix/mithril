//! Specialization by interaction rules: every function reduced over unknown
//! parameters, residual net read back to Core. The oracle is `eval_core`:
//! a specialized module must compute what the original computes.

use mithril_front::core::{Core, CoreModule, Prim, Val};
use mithril_front::{desugar, eval_core, parse};
use mithril_net::specialize;
use std::path::Path;

const FUEL: u64 = 1 << 24;

fn cm(src: &str) -> CoreModule {
    let mut m = parse(src).unwrap_or_else(|d| panic!("parse error line {}: {}", d.line, d.msg));
    let _ = mithril_reassoc::analyze(&mut m);
    desugar(&m).unwrap_or_else(|d| panic!("desugar error line {}: {}", d.line, d.msg))
}

fn oracle(m: &CoreModule) -> Val {
    let m = m.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || eval_core(&m, m.main, &[]))
        .unwrap()
        .join()
        .unwrap()
}

fn spec(src: &str) -> (CoreModule, CoreModule) {
    let m = cm(src);
    let (s, _) = specialize(&m, FUEL);
    assert_eq!(s.fns.len(), m.fns.len(), "specialization must keep every function");
    (m, s)
}

fn body<'a>(m: &'a CoreModule, name: &str) -> &'a Core {
    &m.fns.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no fn {name}")).body
}

fn has<F: Fn(&Core) -> bool + Copy>(e: &Core, p: F) -> bool {
    if p(e) {
        return true;
    }
    match e {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => has(a, p) || has(b, p),
        Core::If(a, b, c) => has(a, p) || has(b, p) || has(c, p),
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().any(|x| has(x, p)),
        Core::Match(s, arms) => has(s, p) || arms.iter().any(|(_, _, b)| has(b, p)),
        Core::Proj(a, _) | Core::Lam(_, a) => has(a, p),
        Core::App(f, a) => has(f, p) || has(a, p),
    }
}

fn calls(e: &Core, g: u32) -> bool {
    has(e, |c| matches!(c, Core::Call(f, _) if *f == g))
}

fn fid(m: &CoreModule, name: &str) -> u32 {
    m.fns.iter().position(|f| f.name == name).unwrap() as u32
}

// ---- semantics over every fixture and corpus program ----

#[test]
fn every_fixture_specializes_to_the_same_value() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mithril-codegen/tests/fixtures");
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|e| e.to_str()) != Some("py") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        let m = match parse(&src) {
            Ok(mut m) => {
                let _ = mithril_reassoc::analyze(&mut m);
                match desugar(&m) {
                    Ok(cm) => cm,
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        };
        // the interpreter oracle is slow on long loops: fixtures it cannot
        // evaluate within a few seconds are covered end to end by the
        // codegen tests (generated code vs oracle), not here
        let t = std::time::Instant::now();
        let want = oracle(&m);
        if t.elapsed() > std::time::Duration::from_secs(3) {
            eprintln!("skipping {} (oracle too slow at net level)", p.display());
            continue;
        }
        let (s, _) = specialize(&m, FUEL);
        assert_eq!(oracle(&s), want, "specialized {} computes a different value", p.display());
        n += 1;
    }
    assert!(n >= 10, "only {n} fixtures checked");
}

// ---- what the rules do ----

#[test]
fn constants_fold_and_known_branches_select() {
    let src = "def f(x):\n    a = 3 * 4 + 5\n    if a > 10:\n        return x + a\n    return x - 1\n\ndef main():\n    return f(7)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    // the branch on a known condition is gone, the constant is folded
    assert!(!has(f, |c| matches!(c, Core::If(..))), "known branch not selected: {f:?}");
    assert!(has(f, |c| matches!(c, Core::Num(17))), "constant not folded: {f:?}");
    // main is fully static: it is a value
    assert!(matches!(body(&s, "main"), Core::Num(24)), "main not reduced to its value: {:?}", body(&s, "main"));
}

#[test]
fn unknown_branches_stay_with_both_arms() {
    let src = "def f(x, y):\n    a = x * 3\n    if y > 2:\n        return a + a\n    return a - 1\n\ndef main():\n    return f(7, 1) + f(2, 9)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    assert!(has(f, |c| matches!(c, Core::If(..))), "branch on a parameter must stay: {f:?}");
    // `a` used in both arms and computed unconditionally: one shared binding
    let lets = count(f, |c| matches!(c, Core::Let(..)));
    assert!(lets >= 1, "shared value not let-bound: {f:?}");
}

fn count<F: Fn(&Core) -> bool + Copy>(e: &Core, p: F) -> usize {
    let here = p(e) as usize;
    here + match e {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => 0,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => count(a, p) + count(b, p),
        Core::If(a, b, c) => count(a, p) + count(b, p) + count(c, p),
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().map(|x| count(x, p)).sum(),
        Core::Match(s, arms) => count(s, p) + arms.iter().map(|(_, _, b)| count(b, p)).sum::<usize>(),
        Core::Proj(a, _) | Core::Lam(_, a) => count(a, p),
        Core::App(f, a) => count(f, p) + count(a, p),
    }
}

#[test]
fn small_acyclic_callees_inline_and_loops_stay_calls() {
    let src = "def sq(x):\n    return x * x\n\ndef f(n):\n    s = 0\n    for i in range(n):\n        s = s + sq(i)\n    return s\n\ndef main():\n    return f(10)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let helper = s.fns.iter().find(|f| f.name.starts_with("__for")).unwrap();
    // sq is inlined into the loop body; the loop's self call is kept
    assert!(!calls(&helper.body, fid(&s, "sq")), "sq not inlined: {:?}", helper.body);
    let hid = s.fns.iter().position(|f| f.name.starts_with("__for")).unwrap() as u32;
    assert!(calls(&helper.body, hid), "loop back-edge lost: {:?}", helper.body);
    assert!(helper.self_tail_rec);
    assert!(helper.fold.is_some(), "fold info must survive specialization");
}

#[test]
fn division_by_zero_stays_at_runtime() {
    // a static division by zero must not fold (and must not crash the compiler)
    let src = "def f(x):\n    return x + 10 // 0\n\ndef main():\n    return 1\n";
    let (_m, s) = spec(src);
    let f = body(&s, "f");
    assert!(has(f, |c| matches!(c, Core::Op2(mithril_front::ast::BinOp::FloorDiv, ..))), "division not kept: {f:?}");
}

#[test]
fn f32_folds_and_arrays_stay_opaque() {
    let src = "def f(a):\n    x = f32_add(1065353216, 1065353216)\n    b = array_set(a, 2, x)\n    return array_get(b, 2) + array_len(b)\n\ndef main():\n    return f(array_new(4, 0))\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    // 1.0f + 1.0f folded to the bit pattern of 2.0f
    assert!(has(f, |c| matches!(c, Core::Num(1073741824))), "f32 add not folded: {f:?}");
    assert!(!has(f, |c| matches!(c, Core::Prim(Prim::F32Add, _))));
    for p in [Prim::ArrSet, Prim::ArrGet, Prim::ArrLen] {
        assert!(has(f, |c| matches!(c, Core::Prim(q, _) if *q == p)), "{p:?} not kept: {f:?}");
    }
    // main: array_new is opaque, so main stays a call chain, not a value
    assert!(!matches!(body(&s, "main"), Core::Num(_)));
}

#[test]
fn matches_on_known_and_unknown_constructors() {
    let src = "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\ndef hd(l):\n    match l:\n        case Nil():\n            return 0\n        case Cons(h, t):\n            return h\n\ndef f(x):\n    return hd(Cons(x, Nil())) + hd(Cons(5, Nil()))\n\ndef g(l):\n    return hd(l) * 2\n\ndef main():\n    return f(3) + g(Cons(1, Nil()))\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    // known constructors: the match selected its arm, no Match remains
    let f = body(&s, "f");
    assert!(!has(f, |c| matches!(c, Core::Match(..))), "match on a known ctor kept: {f:?}");
    // unknown scrutinee: the match stays with both arms
    let g = body(&s, "g");
    assert!(has(g, |c| matches!(c, Core::Match(_, arms) if arms.len() == 2)), "match on a parameter lost: {g:?}");
}

#[test]
fn static_recursion_inside_a_function_reduces() {
    // a call-free recursive computation over constants inside f reduces
    // to its value even though `fact` is cyclic (its call site is fully
    // known once f's own parameter does not flow into it)
    let src = "def fact(n):\n    if n == 0:\n        return 1\n    return n * fact(n - 1)\n\ndef f(x):\n    return x + fact(5)\n\ndef main():\n    return f(1)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    assert!(has(f, |c| matches!(c, Core::Num(120))), "static recursive call not reduced: {f:?}");
}

// ---- speculative unfolding: static control, bounded ----

#[test]
fn static_loop_with_a_runtime_branch_inside_unrolls() {
    // the bound is static, the branch in the body is not: the loop is
    // still unrolled (every iteration's branch stays, its arms
    // instantiated), and no call to the loop helper remains
    let src = "def bitmix(b):\n    s = 0\n    for k in range(6):\n        if (b >> k) & 1 == 1:\n            s = (s + k * 3) & 1048575\n        else:\n            s = s ^ (k + 1)\n    return s\n\ndef main():\n    return bitmix(37) + bitmix(6)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "bitmix");
    assert!(!has(f, |c| matches!(c, Core::Call(..))), "loop helper call kept: {f:?}");
    assert!(has(f, |c| matches!(c, Core::If(..))), "runtime branch lost: {f:?}");
    // one state variable: the loop returns the value itself, no tuple
    assert!(!has(f, |c| matches!(c, Core::Tuple(_) | Core::Proj(..))), "one-variable loop state boxed: {f:?}");
}

#[test]
fn a_loop_calling_a_static_chain_unrolls_both() {
    // a static-bound loop whose body calls a static-count recursion:
    // both unroll (the nested call is unfolded inside the speculation)
    let src = "def spin(n, x):\n    if n == 0:\n        return x\n    return spin(n - 1, (x * 31 + n) & 65535)\n\ndef f(x):\n    acc = 0\n    for i in range(4):\n        acc = (acc + spin(3, x + i)) & 65535\n    return acc\n\ndef main():\n    return f(9)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    assert!(!has(f, |c| matches!(c, Core::Call(..))), "a call remains: {f:?}");
}

#[test]
fn dynamic_control_and_data_builders_are_not_unfolded() {
    // collatz branches on its runtime argument: stays a call. gen builds
    // a list: static count, but a partial unfold would leave constructors
    // over unknown fields (allocation code), so it stays a call too
    let src = "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\ndef collatz(x, n):\n    if x == 1:\n        return n\n    if (x & 1) == 0:\n        return collatz(x >> 1, n + 1)\n    return collatz(3 * x + 1, n + 1)\n\ndef gen(k, s):\n    if k == 0:\n        return Nil()\n    return Cons(s * k, gen(k - 1, s ^ k))\n\ndef total(l):\n    match l:\n        case Nil():\n            return 0\n        case Cons(h, t):\n            return h + total(t)\n\ndef f(x):\n    return collatz(x, 0) + total(gen(5, x))\n\ndef main():\n    return f(27)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    assert!(calls(f, fid(&s, "collatz")), "dynamic recursion unfolded: {f:?}");
    assert!(calls(f, fid(&s, "gen")), "data builder unfolded: {f:?}");
    assert!(!has(f, |c| matches!(c, Core::Ctor(..))), "constructors left in f: {f:?}");
}

#[test]
fn unfolding_is_bounded_by_growth() {
    // a 4000-iteration static loop is not unrolled: the budget stops the
    // chain, the loop stays a call (and computes the same value)
    let src = "def f(x):\n    s = 0\n    for i in range(4000):\n        s = (s * 7 + (x ^ i)) & 1048575\n    return s\n\ndef main():\n    return f(3)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    assert!(has(f, |c| matches!(c, Core::Call(..))), "a 4000-iteration loop was unrolled: {} lets", count(f, |c| matches!(c, Core::Let(..))));
}

#[test]
fn readback_nests_single_use_scalars_and_binds_calls() {
    // single-use ops nest as expressions (the shape codegen's cost models
    // read); a call's result is let-bound where it is used
    let src = "def g(a):\n    if a > 100:\n        return a - 100\n    return g(a * 2)\n\ndef f(x, y):\n    return (x * 3 + y) & 255 + g(x)\n\ndef main():\n    return f(5, 6)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    let lets = count(f, |c| matches!(c, Core::Let(..)));
    assert_eq!(lets, 1, "expected exactly the call bound, got: {f:?}");
    assert!(has(f, |c| matches!(c, Core::Let(_, r, _) if matches!(**r, Core::Call(..)))), "call not bound: {f:?}");
}

// ---- the thesis on a spike-5 shape: an interpreter over a static program ----

#[test]
fn interpreter_over_a_static_program_specializes_to_its_arithmetic() {
    // first Futamura projection by the rules alone: `ev` over a program
    // that is a constant AST and an environment binding the dynamic input
    // leaves no match, no call and no constructor: the residual is the
    // program's arithmetic over the input
    let src = "@data\nclass E:\n    Lit: (n,)\n    Var: (i,)\n    Add: (a, b)\n    Mul: (a, b)\n    If: (c, t, f)\n    Let: (i, v, b)\n\n@data\nclass Env:\n    Emp: ()\n    Bind: (i, v, rest)\n\ndef look(env, i):\n    match env:\n        case Emp():\n            return 0\n        case Bind(j, v, rest):\n            if i == j:\n                return v\n            return look(rest, i)\n\ndef ev(e, env):\n    match e:\n        case Lit(n):\n            return n\n        case Var(i):\n            return look(env, i)\n        case Add(a, b):\n            return (ev(a, env) + ev(b, env)) & 1048575\n        case Mul(a, b):\n            return (ev(a, env) * ev(b, env)) & 1048575\n        case If(c, t, f):\n            if ev(c, env) != 0:\n                return ev(t, env)\n            return ev(f, env)\n        case Let(i, v, b):\n            return ev(b, Bind(i, ev(v, env), env))\n\ndef prog():\n    return Let(1, Mul(Var(0), Lit(3)), If(Var(1), Add(Var(1), Mul(Var(0), Var(0))), Lit(7)))\n\ndef run(x):\n    return ev(prog(), Bind(0, x, Emp()))\n\ndef main():\n    return run(5) + run(0) + run(1000)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let r = body(&s, "run");
    assert!(!has(r, |c| matches!(c, Core::Call(..))), "interpreter call remains: {r:?}");
    assert!(!has(r, |c| matches!(c, Core::Match(..))), "dispatch remains: {r:?}");
    assert!(!has(r, |c| matches!(c, Core::Ctor(..))), "program data remains: {r:?}");
    // the program's own branch on the runtime value is what is left
    assert!(has(r, |c| matches!(c, Core::If(..))), "the program's branch is gone: {r:?}");
    assert!(has(r, |c| matches!(c, Core::Op2(mithril_front::ast::BinOp::Mul, ..))), "the program's arithmetic is gone: {r:?}");
}

// ---- closures at compile time ----

#[test]
fn known_closures_apply_and_factories_specialize() {
    let src = "def mk(k):\n    return lambda x: x * k + 1\n\ndef f(a):\n    g = mk(3)\n    return g(a) + g(2)\n\ndef main():\n    return f(4)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let f = body(&s, "f");
    // mk(3) unfolded, both applications reduced: no closure, no call left
    assert!(!has(f, |c| matches!(c, Core::Lam(..) | Core::App(..) | Core::Call(..))), "{f:?}");
    assert!(has(f, |c| matches!(c, Core::Num(7))), "g(2) = 7 not folded: {f:?}");
}

#[test]
fn unknown_closure_stays_an_application_and_a_returned_closure_a_lambda() {
    let src = "def apply(f, x):\n    return f(x) + f(1)\n\ndef adder(k):\n    return lambda x: x + k\n\ndef main():\n    return apply(adder(2), 3)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let ap = body(&s, "apply");
    assert!(has(ap, |c| matches!(c, Core::App(..))), "application of a parameter lost: {ap:?}");
    let ad = body(&s, "adder");
    assert!(matches!(ad, Core::Lam(..)), "returned closure not a lambda: {ad:?}");
}

#[test]
fn work_free_of_the_parameter_is_bound_outside_the_residual_closure() {
    // mk(k) with k unknown: heavy(k) stays a call, but it is bound once
    // outside the lambda (shared by every application), not inside it
    let src = "def heavy(k):\n    if k < 2:\n        return k\n    return heavy(k - 1) + heavy(k - 2)\n\ndef mk(k):\n    return lambda x: x + heavy(k)\n\ndef main():\n    g = mk(10)\n    return g(1) + g(2)\n";
    let (m, s) = spec(src);
    assert_eq!(oracle(&s), oracle(&m));
    let mk = body(&s, "mk");
    match mk {
        Core::Let(_, r, b) => {
            assert!(matches!(&**r, Core::Call(..)), "heavy not bound outside: {mk:?}");
            assert!(matches!(&**b, Core::Lam(_, lb) if !has(lb, |c| matches!(c, Core::Call(..)))), "heavy inside the closure: {mk:?}");
        }
        other => panic!("expected let heavy in lambda, got {other:?}"),
    }
}
