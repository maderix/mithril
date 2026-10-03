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

/// The per-function pass alone (the shapes below are about one body's
/// readback); `specialize` also clones callees on known arguments.
fn spec(src: &str) -> (CoreModule, CoreModule) {
    let m = cm(src);
    let (s, _) = mithril_net::reduce::specialize_fns(&m, FUEL);
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
        // fixtures whose arrays are too large for the oracle (it copies an
        // array per update) carry an independently computed value instead
        if p.file_name().is_some_and(|f| f == "fill_uninit.py") {
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

// ---- readback keeps the net's independence (a call is bound where the
// net created it, not where its result is first used) ----

/// Every `Call(g, ..)` in `e` that sits under a `Match` whose scrutinee is
/// the result of another call to `g`: a false dependency between two
/// independent calls.
fn call_under_match_of_call(e: &Core, g: u32, call_vars: &mut Vec<u32>) -> bool {
    match e {
        Core::Let(x, r, b) => {
            let under = call_under_match_of_call(r, g, call_vars);
            let pushed = matches!(&**r, Core::Call(f, _) if *f == g);
            if pushed {
                call_vars.push(*x);
            }
            let res = under || call_under_match_of_call(b, g, call_vars);
            if pushed {
                call_vars.pop();
            }
            res
        }
        Core::Match(s, arms) => {
            let on_call = matches!(&**s, Core::Var(v) if call_vars.contains(v));
            arms.iter().any(|(_, _, b)| (on_call && calls(b, g)) || call_under_match_of_call(b, g, call_vars))
        }
        _ => e.kids().into_iter().any(|k| call_under_match_of_call(k, g, call_vars)),
    }
}

/// bitonic's `warp`: two independent recursive calls whose results are
/// zipped by a function that matches the first before the second. The
/// net fires both calls as soon as their arguments exist; the residual
/// body must bind both before matching either, or the second waits on the
/// first and the fork is serialized (measured on the GPU: 1454 ms -> 12 ms
/// on a 2^16-leaf warp once both are bound first).
#[test]
fn independent_calls_stay_independent_through_an_inlined_match() {
    let src = r#"
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def warp_zip(wa, wb):
    match wa:
        case Leaf(av):
            return Leaf(0)
        case Node(a0, a1):
            match wb:
                case Leaf(bv):
                    return Leaf(0)
                case Node(b0, b1):
                    return Node(Node(a0, b0), Node(a1, b1))

def warp(a, b):
    match a:
        case Leaf(av):
            return Node(Leaf(av), a)
        case Node(aa, ab):
            match b:
                case Leaf(bv):
                    return Leaf(0)
                case Node(ba, bb):
                    return warp_zip(warp(aa, ba), warp(ab, bb))

def build(n):
    if n == 0:
        return Leaf(n)
    return Node(build(n - 1), build(n - 1))

def count(t):
    match t:
        case Leaf(v):
            return 1
        case Node(a, b):
            return count(a) + count(b)

def main():
    n = array_len(array_new(5, 0))
    return count(warp(build(n), build(n)))
"#;
    let (m, s) = spec(src);
    assert_eq!(oracle(&m), oracle(&s), "specialized warp changed the result");
    let w = fid(&s, "warp");
    assert!(
        !call_under_match_of_call(body(&s, "warp"), w, &mut Vec::new()),
        "the second recursive warp waits on a match of the first:\n{:?}",
        body(&s, "warp")
    );
}

/// The hoist must not turn tail calls into let-bound calls: a self tail
/// call stays in tail position (loops), a mutual tail call too.
#[test]
fn tail_calls_stay_in_tail_position() {
    let src = r#"
def ev(n, c):
    if n == 0:
        return c
    return od(n - 1, c + 2)

def od(n, c):
    if n == 0:
        return c + 1
    return ev(n - 1, (c * 3) & 1048575)

def main():
    s = 0
    for k in range(4):
        s = (s + ev(3000 + k, k)) & 4294967295
    return s
"#;
    let (m, s) = spec(src);
    assert_eq!(oracle(&m), oracle(&s));
    fn tail_call_to(e: &Core, g: u32) -> bool {
        match e {
            Core::Call(f, _) => *f == g,
            Core::Let(_, _, b) => tail_call_to(b, g),
            Core::If(_, t, f) => tail_call_to(t, g) || tail_call_to(f, g),
            Core::Match(_, arms) => arms.iter().any(|(_, _, b)| tail_call_to(b, g)),
            _ => false,
        }
    }
    let lp = s.fns.iter().position(|f| f.name.starts_with("__for")).expect("loop helper") as u32;
    assert!(tail_call_to(&s.fns[lp as usize].body, lp), "the loop's back-edge is no longer a tail call: {:?}", s.fns[lp as usize].body);
    let (e, o) = (fid(&s, "ev"), fid(&s, "od"));
    assert!(tail_call_to(body(&s, "ev"), o), "ev's call to od is no longer a tail call");
    assert!(tail_call_to(body(&s, "od"), e), "od's call to ev is no longer a tail call");
}

/// Every variable a specialized body reads is bound (a parameter, a let,
/// a match binder, a lambda parameter): nothing was hoisted out of the
/// scope that binds it.
fn assert_scoped(s: &CoreModule) {
    for f in &s.fns {
        let free = f.body.free_vars();
        assert!(free.iter().all(|v| (*v as usize) < f.arity), "{}: reads unbound {:?} in {:?}", f.name, free, f.body);
    }
}

const FIB: &str = "def fib(k):\n    if k < 2:\n        return k\n    return fib(k - 1) + fib(k - 2)\n\n";

/// A call inside a closure body stays in the closure when its argument
/// reads the parameter, directly or through a value bound inside the
/// closure (review of 01d2609: `lambda x: fib(fib(x) + k)` escaped).
#[test]
fn calls_in_closure_bodies_stay_in_scope() {
    for body in [
        "lambda x: fib(fib(x) + k)",
        "lambda x: fib(fib(x) + k) + 1",
        "lambda x: fib(k + fib(pair(x, k)[0]))",
    ] {
        let src = format!("{FIB}def pair(a, b):\n    return (a, b)\n\ndef mk(k):\n    return {body}\n\ndef main():\n    g = mk(1)\n    return g(5) + g(6)\n");
        let (m, s) = spec(&src);
        assert_scoped(&s);
        assert_eq!(oracle(&m), oracle(&s), "{body}");
    }
}

/// A call created only inside a branch stays in the branch, also when an
/// identical nullary call runs in an outer frame (review of 01d2609: all
/// nullary calls to one function shared a key, so the branch-only one was
/// hoisted and evaluated on every path).
#[test]
fn branch_only_calls_stay_in_their_branch() {
    let src = r#"
def z():
    return w(array_len(array_new(3, 0)))

def w(n):
    if n == 0:
        return 0
    if n > 100:
        return z()
    return w(n - 1) + 1

def f(c, d):
    a = z()
    if c > 0:
        if d > 0:
            return a + z()
        return a
    return 0

def main():
    return f(1, 1) + f(1, 0) + f(0, 1)
"#;
    let (m, s) = spec(src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
    let z = fid(&s, "z");
    // outside every branch of f: exactly the one unconditional call
    fn top_calls(e: &Core, g: u32) -> usize {
        match e {
            Core::Let(_, r, b) => top_calls(r, g) + top_calls(b, g),
            Core::Call(f, xs) => (*f == g) as usize + xs.iter().map(|x| top_calls(x, g)).sum::<usize>(),
            Core::If(..) | Core::Match(..) => 0,
            _ => e.kids().into_iter().map(|k| top_calls(k, g)).sum(),
        }
    }
    // the unconditional z() may be read back inside the branch that uses it
    // (expose.rs), but the branch-only one never moves out of its branch
    assert!(top_calls(body(&s, "f"), z) <= 1, "a branch-only z() was hoisted: {:?}", body(&s, "f"));
}

/// A call whose result is shared (a Dup) inside an arm stays in the arm.
#[test]
fn shared_call_in_an_arm_stays_in_the_arm() {
    let src = format!("{FIB}def f(n, c):\n    if c > 0:\n        y = fib(n)\n        return y * y + y\n    return 0\n\ndef main():\n    return f(10, 1) + f(3, 0)\n");
    let (m, s) = spec(&src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
    let fb = fid(&s, "fib");
    match body(&s, "f") {
        Core::If(_, t, e) => assert!(calls(t, fb) && !calls(e, fb), "fib left its arm: {:?}", body(&s, "f")),
        other => panic!("f is no longer a branch at the top: {other:?}"),
    }
}

/// A shared value computed inside a closure from its parameter stays in
/// the closure, also under an arm (second review of the placement: a Dup
/// inside a closure body is recorded in the enclosing settle frame, and
/// reading it there used to hide the closure).
#[test]
fn shared_values_in_closure_bodies_stay_in_scope() {
    for (mk, apply) in [
        ("def mk(k):\n    return lambda x: sq(fib(x) + k)\n", "g = mk(1)\n    return g(5) + g(6)"),
        ("def mk(k, c):\n    if c > 0:\n        return lambda x: sq(fib(x) + k)\n    return lambda x: x\n", "g = mk(1, 1)\n    h = mk(1, 0)\n    return g(5) + g(6) + h(7)"),
    ] {
        let src = format!("{FIB}def sq(y):\n    return y * y\n\n{mk}\ndef main():\n    {apply}\n");
        let (m, s) = spec(&src);
        assert_scoped(&s);
        assert_eq!(oracle(&m), oracle(&s), "{mk}");
    }
}

/// A call the net computes once outside a closure stays outside it, also
/// when the closure uses it under an arm: every application shares it
/// (the DUP sharing the language exists for; the second review found a
/// clamp that rebuilt it inside the closure, once per application).
#[test]
fn a_call_outside_a_closure_is_shared_by_every_application() {
    let src = format!("{FIB}def pick(x, k):\n    if x > 0:\n        return k + 1\n    return 0\n\ndef mk(n):\n    k = fib(n)\n    return lambda x: pick(x, k)\n\ndef main():\n    g = mk(12)\n    return g(1) + g(2) + g(0) + g(3)\n");
    let (m, s) = spec(&src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
    let fb = fid(&s, "fib");
    fn call_under_lam(e: &Core, g: u32, under: bool) -> bool {
        match e {
            Core::Call(f, _) if *f == g => under,
            Core::Lam(_, b) => call_under_lam(b, g, true),
            _ => e.kids().into_iter().any(|k| call_under_lam(k, g, under)),
        }
    }
    let b = body(&s, "mk");
    assert!(calls(b, fb) && !call_under_lam(b, fb, false), "fib(n) is recomputed per application: {b:?}");
}

// ---- placement follows what a value reads; sharing per selection (review of ba0c48b) ----

#[test]
fn a_closure_applied_twice_at_compile_time_shares_nothing_across_applications() {
    // g(1) and g(k) read the same Dup through different superposition
    // sides; the shared binding must be per selection, or g(k) reuses g(1)
    let src = format!("{FIB}def sq(y):\n    return y * y\n\ndef f(k, c):\n    g = lambda x: sq(fib(x) + c)\n    return g(1) + g(k)\n\ndef main():\n    k = array_len(array_new(9, 0))\n    return f(k, 0)\n");
    let (m, s) = spec(&src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
}

#[test]
fn a_call_created_in_a_branch_stays_in_the_branch() {
    // a closure body's call is applied inside a branch: it binds there
    // (down diverges for a negative argument: moving the call out of the
    // branch changes termination). Three shapes: the call's argument is
    // made in the branch; it is a parameter (the closure body's call was
    // reachable, unapplied, from the function frame); the branch is a
    // match arm.
    const DOWN: &str = "def down(n):\n    if n == 0:\n        return 0\n    return down(n - 1) + 1\n\n";
    for f_src in [
        "def f(k, c):\n    g = lambda x: fib(x) + k\n    if c > 0:\n        t = down(c)\n        return g(t)\n    return 0\n\ndef main():\n    k = array_len(array_new(9, 0))\n    return f(k, k - 10) + f(k, 3)\n",
        "def f(k, c):\n    g = lambda x: down(x) + k\n    if c < 0:\n        return 0\n    return g(c)\n\ndef main():\n    k = array_len(array_new(9, 0))\n    return f(k, k - 10) + f(k, k)\n",
        "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\ndef f(k, l, c):\n    g = lambda x: down(x) + k\n    match l:\n        case Cons(h, r):\n            return h\n        case Nil():\n            return g(c)\n\ndef main():\n    k = array_len(array_new(9, 0))\n    return f(k, Cons(k, Nil()), k - 10) + f(k, Nil(), k)\n",
    ] {
        let src = format!("{DOWN}{FIB}{f_src}");
        let (m, s) = spec(&src);
        assert_scoped(&s);
        assert_eq!(oracle(&m), oracle(&s), "{f_src}");
        let b = body(&s, "f");
        let dn = fid(&s, "down");
        fn under_branch(e: &Core, g: u32, under: bool) -> bool {
            match e {
                Core::Call(f, _) if *f == g => under,
                Core::If(c, t, f) => under_branch(c, g, under) || under_branch(t, g, true) || under_branch(f, g, true),
                Core::Match(sc, arms) => under_branch(sc, g, under) || arms.iter().any(|(_, _, a)| under_branch(a, g, true)),
                _ => e.kids().into_iter().any(|k| under_branch(k, g, under)),
            }
        }
        assert!(under_branch(b, dn, false), "down left its branch in {f_src}: {b:?}");
    }
}

/// Open (the reviewer's probe q2b): a closure created in an arm that
/// captures a pattern binder, applied twice, hits "residual wire class has
/// no producer" in the reader.
#[test]
#[ignore = "open: ICE in the reader (design.md, open items)"]
fn a_closure_capturing_an_arm_binder_applied_twice() {
    let src = format!("@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\n{FIB}def f(k, l, c):\n    match l:\n        case Cons(h, t):\n            g = lambda x: fib(x + h) + k\n            return g(c) + g(1)\n        case Nil():\n            return 0\n\ndef main():\n    k = array_len(array_new(9, 0))\n    return f(k, Cons(k, Nil()), k) + f(k, Nil(), k)\n");
    let (m, s) = spec(&src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
}

#[test]
fn an_arm_binder_flowing_into_a_closure_body_binds_after_the_match() {
    for body_src in ["fib(x) + k", "sq(fib(x) + k)"] {
        let src = format!("@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\n{FIB}def sq(y):\n    return y * y\n\ndef f(k, l):\n    g = lambda x: {body_src}\n    match l:\n        case Cons(h, t):\n            return g(h)\n        case Nil():\n            return 0\n\ndef main():\n    k = array_len(array_new(9, 0))\n    return f(k, Cons(k, Nil())) + f(k, Nil())\n");
        let (m, s) = spec(&src);
        assert_scoped(&s);
        assert_eq!(oracle(&m), oracle(&s), "{body_src}");
    }
}

#[test]
fn failed_static_evaluations_are_charged_to_the_speculation_budget() {
    // `run(600)` unrolls a known loop whose iterations call `build` and
    // `comps` with known arguments; each call is too large to finish in a
    // scratch net. Without a charge, every iteration retried its calls and
    // compilation took minutes; with one, the calls stay residual.
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/general/graph_dfs.py")).unwrap();
    let m = desugar(&parse(&src).unwrap()).unwrap();
    let t = std::time::Instant::now();
    let (s, _) = specialize(&m, mithril_net::REDUCE_FUEL);
    assert!(t.elapsed().as_secs() < 30, "specialization took {:?}", t.elapsed());
    // `run(600)` may call a clone of `run` specialized on 600 (known.rs)
    let runs: Vec<u32> = s.fns.iter().enumerate().filter(|(_, f)| f.name == "run" || f.name.starts_with("run_")).map(|(i, _)| i as u32).collect();
    assert!(runs.iter().any(|r| calls(body(&s, "main"), *r)), "the large call stays a call");
}

// ---- readback order that exposes forks (expose.rs) ----

fn calls_to(e: &Core, g: u32) -> usize {
    let mut n = 0;
    e.walk(&mut |x| if matches!(x, Core::Call(f, _) if *f == g) { n += 1 });
    n
}

#[test]
fn a_call_before_an_independent_test_is_read_back_inside_its_arms() {
    // subsetsum's shape: the test reads w, not skip; one arm calls count again
    let src = "def weight(i):\n    return (i * 7 + 3) & 15\n\ndef count(i, n, room):\n    if i == n:\n        return 1\n    skip = count(i + 1, n, room)\n    w = weight(i)\n    if w > room:\n        return skip\n    take = count(i + 1, n, room - w)\n    return (skip + take) & 4294967295\n\ndef main():\n    return count(0, array_len(array_new(12, 0)), 40)\n";
    let (m, s) = spec(src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
    let c = fid(&s, "count");
    // the test comes before any recursive call: find the If on w > room
    fn first_if(e: &Core) -> Option<&Core> {
        match e {
            Core::If(..) => Some(e),
            Core::Let(_, _, b) => first_if(b),
            _ => None,
        }
    }
    let Core::If(_, _, rest) = body(&s, "count") else { panic!("count does not start with its base case") };
    let Some(Core::If(test, then, els)) = first_if(rest) else { panic!("no test after the base case: {rest:?}") };
    assert!(!has(test, |x| matches!(x, Core::Call(g, _) if *g == c)), "the test waits for a recursive call");
    assert_eq!(calls_to(then, c), 1, "the pruned arm makes one call: {then:?}");
    assert_eq!(calls_to(els, c), 2, "the full arm makes two calls: {els:?}");
    // in the full arm the two calls are independent: neither argument reads the other's binder
    let Core::Let(x, r1, b1) = &**els else { panic!("full arm does not bind its first call: {els:?}") };
    let Core::Let(_, r2, _) = &**b1 else { panic!("full arm does not bind its second call: {els:?}") };
    assert!(matches!(**r1, Core::Call(g, _) if g == c) && matches!(**r2, Core::Call(g, _) if g == c));
    assert!(!r2.reads(*x), "the second call reads the first: {els:?}");
}

#[test]
fn calls_stay_where_the_test_needs_them() {
    // the test reads the call's result: nothing to move
    let reads = "def f(i):\n    return i * 3 + 1\n\ndef g(n, k):\n    x = f(n)\n    if x > k:\n        return f(n + 1) + x\n    return x\n\ndef main():\n    n = array_len(array_new(5, 0))\n    return g(n, 3) + g(n, 99)\n";
    let (m, s) = spec(reads);
    assert_eq!(oracle(&m), oracle(&s));
    // no arm calls anything independent: nothing to expose
    let plain = "def f(i):\n    if i < 1:\n        return 1\n    return f(i - 1) + i\n\ndef g(n, c):\n    x = f(n)\n    if c > 0:\n        return x\n    return x + 1\n\ndef main():\n    n = array_len(array_new(6, 0))\n    return g(n, 1) + g(n, 0)\n";
    let (m2, s2) = spec(plain);
    assert_eq!(oracle(&m2), oracle(&s2));
    let gf = fid(&s2, "f");
    assert!(matches!(body(&s2, "g"), Core::Let(_, r, _) if matches!(**r, Core::Call(h, _) if h == gf)), "g's call moved without exposing a fork: {:?}", body(&s2, "g"));
}

#[test]
fn an_arm_that_ignores_the_call_gets_no_binding() {
    // the net erases a call whose result is unused; the readback places none there
    let src = "def f(i):\n    if i < 1:\n        return 1\n    return f(i - 1) + f(i - 1)\n\ndef g(n, c):\n    x = f(n)\n    if c > 0:\n        return x + f(n + 1)\n    return 7\n\ndef main():\n    n = array_len(array_new(6, 0))\n    return g(n, 1) + g(n, 0)\n";
    let (m, s) = spec(src);
    assert_scoped(&s);
    assert_eq!(oracle(&m), oracle(&s));
    let gf = fid(&s, "f");
    fn find_if(e: &Core) -> Option<(&Core, &Core)> {
        match e {
            Core::If(_, t, f) => Some((t, f)),
            Core::Let(_, _, b) => find_if(b),
            _ => None,
        }
    }
    let (then, els) = find_if(body(&s, "g")).expect("g keeps its test");
    assert_eq!(calls_to(then, gf), 2, "{then:?}");
    assert_eq!(calls_to(els, gf), 0, "the arm returning 7 still calls f: {els:?}");
}

// ---- specialization on known arguments (known.rs) ----

fn full(src: &str) -> (CoreModule, CoreModule) {
    let mut m = parse(src).unwrap_or_else(|d| panic!("parse error line {}: {}", d.line, d.msg));
    let _ = mithril_reassoc::analyze(&mut m);
    let m = desugar(&m).unwrap();
    let (s, _) = specialize(&m, FUEL);
    assert_eq!(oracle(&m), oracle(&s), "specialized module computes a different value");
    (m, s)
}

fn named<'a>(m: &'a CoreModule, prefix: &str) -> Vec<&'a mithril_front::core::CoreFn> {
    m.fns.iter().filter(|f| f.name.starts_with(prefix)).collect()
}

#[test]
fn a_loop_invariant_constant_reaches_the_loop_body() {
    let src = "def scale(n, k):\n    s = 0\n    for i in range(n):\n        s = s * 3 + i % k\n    return s\n\ndef main():\n    n = array_len(array_new(50, 0))\n    return scale(n, 8)\n";
    let (_, s) = full(src);
    // the loop's clone divides by the constant (codegen makes it a mask)
    let modk = |f: &&mithril_front::core::CoreFn| has(&f.body, |e| matches!(e, Core::Op2(mithril_front::ast::BinOp::Mod, _, k) if **k == Core::Num(8)));
    assert!(s.fns.iter().any(|f| modk(&f)), "no function divides by the constant 8: {:?}", s.fns.iter().map(|f| &f.name).collect::<Vec<_>>());
}

#[test]
fn a_parameter_that_changes_is_never_fixed() {
    let src = "def f(n, k):\n    if n == 0:\n        return k\n    return f(n - 1, k + 1)\n\ndef main():\n    n = array_len(array_new(9, 0))\n    return f(n, 0)\n";
    let (_, s) = full(src);
    assert!(named(&s, "f_1k").is_empty(), "k changes in the recursion: {:?}", s.fns.iter().map(|f| &f.name).collect::<Vec<_>>());
}

#[test]
fn mutual_recursion_fixes_nothing() {
    let src = "def ev(n, k):\n    if n == 0:\n        return k\n    return od(n - 1, k)\n\ndef od(n, k):\n    if n == 0:\n        return 0 - k\n    return ev(n - 1, k)\n\ndef main():\n    n = array_len(array_new(7, 0))\n    return ev(n, 5)\n";
    let (m, s) = full(src);
    assert_eq!(s.fns.len(), m.fns.len(), "a parameter through a call cycle is not fixed");
}

#[test]
fn a_proven_fold_keeps_its_counter_bound_and_accumulator() {
    let src = "def total(n, k):\n    s = 0\n    for i in range(n):\n        s = s + i % k\n    return s\n\ndef main():\n    return total(array_len(array_new(100000, 0)), 3) + total(40, 2)\n";
    let (_, s) = full(src);
    let mut fixed_extra = false;
    for f in s.fns.iter().filter(|f| f.fold.is_some()) {
        // a fold clone may fix its extra (k), never its counter, bound or
        // accumulator (the `Var` its loop returns)
        let Core::If(_, _, e) = &f.body else { panic!("{} is no fold shape", f.name) };
        let Core::Var(acc) = **e else { panic!("{} returns no accumulator", f.name) };
        for p in [0, 1, acc] {
            assert!(!f.name.contains(&format!("_{p}k")), "fold {} fixed structural parameter {p}", f.name);
        }
        fixed_extra |= f.name.contains('k');
    }
    assert!(fixed_extra, "the fold's extra k is fixed: {:?}", s.fns.iter().map(|f| &f.name).collect::<Vec<_>>());
}

#[test]
fn clones_do_not_chain_and_originals_no_call_reaches_lose_their_code() {
    let src = "def f(i, d):\n    if i < 1:\n        return 1\n    return f(i - 1, d) + i % d\n\ndef g(n, d):\n    return f(n, d) + 1\n\ndef main():\n    n = array_len(array_new(6, 0))\n    return g(n, 4) + g(n, 4) + g(n, 3)\n";
    let (_, s) = full(src);
    assert_eq!(named(&s, "g_1k4").len(), 1, "one clone per fixed argument, never a clone of a clone: {:?}", s.fns.iter().map(|f| &f.name).collect::<Vec<_>>());
    assert!(named(&s, "g_1k4_").is_empty());
    assert_eq!(named(&s, "f_1k4").len(), 1, "the divisor reaches f's own clone");
    assert_eq!(body(&s, "g"), &Core::Num(0), "g is no longer called");
}
