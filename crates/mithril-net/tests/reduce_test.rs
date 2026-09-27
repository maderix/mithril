//! Task 5 acceptance tests: net build, rule table, compile-time reducer,
//! readback. The semantic oracle is `mithril_front::eval_core`.

use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::core::{CoreModule, Val};
use mithril_front::{desugar, eval_core, parse};
use mithril_net::{build, check_clone_discipline, readback, reduce, root_port, EMPTY};

// ---- helpers ----

fn cm(src: &str) -> CoreModule {
    let m = parse(src).unwrap_or_else(|d| panic!("parse error line {}: {}", d.line, d.msg));
    desugar(&m).unwrap_or_else(|d| panic!("desugar error line {}: {}", d.line, d.msg))
}

fn oracle(m: &CoreModule) -> Val {
    eval_core(m, m.main, &[])
}

/// Build, fully reduce (large fuel), assert readback == oracle and that
/// the net actually quiesced; returns the rewrite count.
fn check(src: &str) -> u64 {
    let m = cm(src);
    let mut net = build(&m);
    let n = reduce(&mut net, &m, 1_000_000);
    assert!(net.redexes.is_empty(), "program did not fully reduce:\n{}", src);
    assert_eq!(
        readback(&net, root_port()),
        Some(oracle(&m)),
        "net readback != eval_core oracle for:\n{}",
        src
    );
    n
}

// ---- arithmetic / comparisons / floats ----

#[test]
fn arithmetic_expr_matches_oracle() {
    check("def main():\n    return (1 + 2 * 3 - 4) * 10\n");
    check("def main():\n    return (0 - 7) // 2 + (0 - 7) % 2\n");
    check("def main():\n    return (123456789 << 30) + (1 << 3) - (256 >> 4)\n");
    check("def main():\n    return (12 & 10) | (5 ^ 3)\n");
}

#[test]
fn comparison_and_bool_ops_match_oracle() {
    check("def main():\n    return 3 < 5\n");
    check("def main():\n    return 5 <= 4\n");
    check("def main():\n    x = 10\n    if x > 5 and x < 20:\n        return 1\n    else:\n        return 2\n");
    check("def main():\n    x = 3\n    if x > 5 or x == 3:\n        return 7\n    else:\n        return 8\n");
    check("def main():\n    if not 1 == 2:\n        return 1\n    else:\n        return 0\n");
}

#[test]
fn float_arithmetic_matches_oracle() {
    check("def main():\n    return 1.5 + 2.25 * 2.0\n");
    check("def main():\n    return 3.0 / 2.0 - 0.25\n");
    check("def main():\n    return 2.5 < 2.75\n");
}

// ---- loops (while / for desugar) ----

#[test]
fn factorial_10_via_while_matches_oracle() {
    let src = "\
def fact(n):
    r = 1
    while n > 0:
        r = r * n
        n = n - 1
    return r

def main():
    return fact(10)
";
    let m = cm(src);
    assert_eq!(oracle(&m), Val::I(3_628_800));
    check(src);
}

#[test]
fn sum_via_for_matches_oracle() {
    check("def main():\n    s = 0\n    for i in range(10):\n        s = s + i\n    return s\n");
}

// ---- constructors / match ----

const TREE: &str = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def mk(d, v):
    if d == 0:
        return Leaf(v)
    return Node(mk(d - 1, v * 2), mk(d - 1, v * 2 + 1))

def sumtree(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            return sumtree(l) + sumtree(r)
";

#[test]
fn tree_sum_depth_4_matches_oracle() {
    let src = format!("{}\ndef main():\n    return sumtree(mk(4, 1))\n", TREE);
    let m = cm(&src);
    assert_eq!(oracle(&m), Val::I((16..32).sum::<i64>()));
    check(&src);
}

#[test]
fn match_with_nested_ctors_matches_oracle() {
    let src = format!(
        "{}\n\
def peek2(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            match l:
                case Leaf(v):
                    return v + sumtree(r)
                case Node(a, b):
                    return sumtree(a) + sumtree(b) + sumtree(r)

def main():
    return peek2(Node(Node(Leaf(1), Leaf(2)), Leaf(3)))
",
        TREE
    );
    check(&src);
}

#[test]
fn constructor_value_readback_matches_oracle() {
    // Result is a data value, not a number: CON chains -> Val::C.
    let src = format!("{}\ndef main():\n    return Node(Leaf(1 + 2), mk(1, 4))\n", TREE);
    let m = cm(&src);
    let expect = oracle(&m);
    assert!(matches!(expect, Val::C(..)));
    check(&src);
}

#[test]
fn tuple_and_proj_match_oracle() {
    check("def main():\n    return (1, 2 + 3, 4)\n");
    check("def main():\n    t = (7, 8)\n    return t[1]\n");
    check("def main():\n    t = (7, (8, 9))\n    return t[1][0] + t[0]\n");
}

// ---- sharing (DUP) ----

#[test]
fn shared_subexpression_reduces_once() {
    // `x` used twice: DUP on the computed value, loop runs once.
    let shared = "\
def sumn(n):
    s = 0
    for i in range(n):
        s = s + i
    return s

def main():
    x = sumn(20)
    return x + x
";
    // Naive form recomputes the loop twice.
    let naive = "\
def sumn(n):
    s = 0
    for i in range(n):
        s = s + i
    return s

def main():
    return sumn(20) + sumn(20)
";
    let n_shared = check(shared);
    let n_naive = check(naive);
    assert!(
        n_shared < n_naive,
        "sharing must compute the subexpression once: shared={} naive={}",
        n_shared,
        n_naive
    );
}

#[test]
fn shared_ctor_value_dup_con_matches_oracle() {
    // A constructor value used twice: DUP-CON lazy copy.
    let src = format!(
        "{}\ndef main():\n    t = Node(Leaf(1), Node(Leaf(2), Leaf(3)))\n    return sumtree(t) * 100 + sumtree(t)\n",
        TREE
    );
    check(&src);
}

// ---- clone discipline ----

#[test]
fn clone_restriction_lambda_rejected_by_front() {
    // v1: lambdas are parse-only; desugar rejects them before the net tier,
    // so the `lambda g: g(g)` double-clone program dies with a front Diag.
    let src = "def main():\n    h = lambda g: g(g)\n    return 0\n";
    let m = parse(src).expect("lambda must parse");
    let err = desugar(&m).expect_err("desugar must reject lambdas in v1");
    assert!(
        err.msg.contains("lambda"),
        "diag must name the lambda restriction, got: {}",
        err.msg
    );
}

#[test]
fn check_clone_discipline_ok_on_first_order_core() {
    let src = format!("{}\ndef main():\n    t = mk(2, 1)\n    return sumtree(t) + sumtree(t)\n", TREE);
    let m = cm(&src);
    assert_eq!(check_clone_discipline(&m), Ok(()));
}

// ---- fuel semantics ----

#[test]
fn fuel_zero_leaves_dump_unchanged() {
    let m = cm("def main():\n    return 1 + 2\n");
    let mut net = build(&m);
    let before = net.dump();
    let n = reduce(&mut net, &m, 0);
    assert_eq!(n, 0, "fuel=0 must perform zero rewrites");
    assert_eq!(net.dump(), before, "fuel=0 must leave the dump unchanged");
    assert_eq!(readback(&net, root_port()), None, "unreduced root reads back as None");
}

#[test]
fn fuel_bounds_rewrite_count() {
    let src = "def fact(n):\n    r = 1\n    while n > 0:\n        r = r * n\n        n = n - 1\n    return r\n\ndef main():\n    return fact(10)\n";
    let m = cm(src);
    let mut net = build(&m);
    let n = reduce(&mut net, &m, 5);
    assert!(n <= 5, "reduce must perform at most `fuel` rewrites, did {}", n);
    assert!(!net.redexes.is_empty(), "fuel-starved net must keep residual redexes");
}

#[test]
fn fuel_starved_reduce_resumes_to_completion() {
    let src = "def fact(n):\n    r = 1\n    while n > 0:\n        r = r * n\n        n = n - 1\n    return r\n\ndef main():\n    return fact(10)\n";
    let m = cm(src);
    let mut net = build(&m);
    let mut whole = build(&m);
    let total = reduce(&mut whole, &m, 1_000_000);
    let first = reduce(&mut net, &m, 37);
    let rest = reduce(&mut net, &m, 1_000_000);
    assert_eq!(first + rest, total, "resumed reduction must do the same total work");
    assert_eq!(readback(&net, root_port()), Some(oracle(&m)));
}

// ---- determinism ----

#[test]
fn reduce_is_deterministic() {
    let src = format!("{}\ndef main():\n    return sumtree(mk(3, 1)) + sumtree(mk(2, 5))\n", TREE);
    let m = cm(&src);

    let mut n1 = build(&m);
    let mut n2 = build(&m);
    assert_eq!(n1.dump(), n2.dump(), "build must be deterministic");

    let c1 = reduce(&mut n1, &m, 1_000_000);
    let c2 = reduce(&mut n2, &m, 1_000_000);
    assert_eq!(c1, c2, "rewrite counts must match");
    assert_eq!(n1.dump(), n2.dump(), "reduced dumps (live cells) must match");
    assert_eq!(readback(&n1, root_port()), readback(&n2, root_port()));

    // Also deterministic under a partial-fuel cut.
    let mut p1 = build(&m);
    let mut p2 = build(&m);
    let d1 = reduce(&mut p1, &m, 53);
    let d2 = reduce(&mut p2, &m, 53);
    assert_eq!(d1, d2);
    assert_eq!(p1.dump(), p2.dump());
}

// ---- net-level rules unreachable from Core v1 ----

#[test]
fn app_lam_beta_net_level() {
    // (lambda x: x)(42) built by hand: LAM/APP exist for net-level
    // completeness only (Core v1 is first-order).
    let m = CoreModule::default();
    let mut net = Net::new();
    let root = net.alloc(EMPTY, EMPTY); // result wire (cell 0)
    let wp = net.alloc(EMPTY, EMPTY); // param wire
    let lam = net.alloc(Port::new(Tag::Var, wp as u64), Port::new(Tag::Var, wp as u64));
    let app = net.alloc(Port::num(42), Port::new(Tag::Var, root as u64));
    net.redexes.push((Port::new(Tag::App, app as u64), Port::new(Tag::Lam, lam as u64)));
    let n = reduce(&mut net, &m, 10);
    assert_eq!(n, 1, "beta is one rewrite");
    assert_eq!(readback(&net, Port::new(Tag::Var, root as u64)), Some(Val::I(42)));
}

#[test]
fn dup_num_net_level() {
    // DUP of an unboxed NUM: both outputs receive the value.
    let m = CoreModule::default();
    let mut net = Net::new();
    let w1 = net.alloc(EMPTY, EMPTY);
    let w2 = net.alloc(EMPTY, EMPTY);
    let d = net.alloc(Port::new(Tag::Var, w1 as u64), Port::new(Tag::Var, w2 as u64));
    // Dup payload: addr:40 | label:16 (single label class, label 0).
    net.redexes.push((Port::new(Tag::Dup, (d as u64) << 16), Port::num(21)));
    let n = reduce(&mut net, &m, 10);
    assert_eq!(n, 1);
    assert_eq!(readback(&net, Port::new(Tag::Var, w1 as u64)), Some(Val::I(21)));
    assert_eq!(readback(&net, Port::new(Tag::Var, w2 as u64)), Some(Val::I(21)));
}

#[test]
fn dup_dup_same_label_annihilates_net_level() {
    let m = CoreModule::default();
    let mut net = Net::new();
    let w1 = net.alloc(EMPTY, EMPTY);
    let w2 = net.alloc(EMPTY, EMPTY);
    let d1 = net.alloc(Port::new(Tag::Var, w1 as u64), Port::new(Tag::Var, w2 as u64));
    let d2 = net.alloc(Port::num(7), Port::num(8));
    net.redexes.push((Port::new(Tag::Dup, (d1 as u64) << 16), Port::new(Tag::Dup, (d2 as u64) << 16)));
    let n = reduce(&mut net, &m, 10);
    assert_eq!(n, 1, "annihilation is one rewrite");
    assert_eq!(readback(&net, Port::new(Tag::Var, w1 as u64)), Some(Val::I(7)));
    assert_eq!(readback(&net, Port::new(Tag::Var, w2 as u64)), Some(Val::I(8)));
}

#[test]
fn era_con_erases_recursively_and_frees_cells() {
    let m = CoreModule::default();
    let mut net = Net::new();
    let a = net.alloc(Port::num(1), Port::num(2));
    net.redexes.push((Port::new(Tag::Era, 0), Port::con(a as u64, 5, 2)));
    let n = reduce(&mut net, &m, 10);
    assert_eq!(n, 3, "ERA-CON then ERA-NUM twice");
    assert_eq!(net.dump(), "", "erasure must free every cell and leave no redexes");
}

#[test]
fn full_reduction_of_scalar_program_leaves_only_the_root_wire() {
    // Linearity: every rule frees what it consumes, so a program with a
    // NUM result must quiesce to exactly one live cell (the root wire).
    for src in [
        "def main():\n    return 1 + 2 * 3\n",
        "def fact(n):\n    r = 1\n    while n > 0:\n        r = r * n\n        n = n - 1\n    return r\n\ndef main():\n    return fact(10)\n",
    ] {
        let m = cm(src);
        let mut net = build(&m);
        reduce(&mut net, &m, 1_000_000);
        let live = net.cells.len() - net.free.len();
        assert_eq!(live, 1, "expected only the root wire to stay live for:\n{}\ndump:\n{}", src, net.dump());
    }
}

#[test]
#[should_panic(expected = "ICE")]
fn unlisted_pair_is_a_named_ice() {
    // NUM-NUM has no rule and is not inert wiring: compile-time ICE.
    let m = CoreModule::default();
    let mut net = Net::new();
    net.redexes.push((Port::num(1), Port::num(2)));
    reduce(&mut net, &m, 10);
}
