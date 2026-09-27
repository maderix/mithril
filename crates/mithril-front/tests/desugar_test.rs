use mithril_front::core::{Core, CoreModule, CtorId};
use mithril_front::lex::{I56_MAX, I56_MIN};
use mithril_front::{desugar, eval_core, parse, Diag, Val};

fn dm(src: &str) -> CoreModule {
    let m = parse(src).unwrap_or_else(|d| panic!("parse error at line {}: {}", d.line, d.msg));
    desugar(&m).unwrap_or_else(|d| panic!("desugar error at line {}: {}", d.line, d.msg))
}

fn desugar_err(src: &str) -> Diag {
    let m = parse(src).unwrap_or_else(|d| panic!("parse error at line {}: {}", d.line, d.msg));
    match desugar(&m) {
        Ok(cm) => panic!("expected desugar error, got: {:?}", cm),
        Err(d) => d,
    }
}

fn fid(cm: &CoreModule, name: &str) -> u32 {
    cm.fns.iter().position(|f| f.name == name).unwrap_or_else(|| panic!("no fn named '{}' in {:?}", name, cm.fns.iter().map(|f| &f.name).collect::<Vec<_>>())) as u32
}

fn helper_named<'a>(cm: &'a CoreModule, prefix: &str) -> &'a mithril_front::core::CoreFn {
    cm.fns.iter().find(|f| f.name.starts_with(prefix)).unwrap_or_else(|| panic!("no helper fn with prefix '{}'", prefix))
}

// ---- required brief scenarios ----

#[test]
fn factorial_via_while_equals_120() {
    let src = "def fact(n):\n    r = 1\n    while n > 0:\n        r = r * n\n        n = n - 1\n    return r\n";
    let cm = dm(src);
    let f = fid(&cm, "fact");
    assert_eq!(eval_core(&cm, f, &[Val::I(5)]), Val::I(120));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(1));
}

#[test]
fn sum_via_for_range_10_equals_45() {
    let src = "def sumn(n):\n    s = 0\n    for i in range(n):\n        s = s + i\n    return s\n";
    let cm = dm(src);
    let f = fid(&cm, "sumn");
    assert_eq!(eval_core(&cm, f, &[Val::I(10)]), Val::I(45));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(0));
}

#[test]
fn match_on_tree_depth_2_sums_leaves() {
    let src = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def sumtree(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            return sumtree(l) + sumtree(r)

def main():
    return sumtree(Node(Node(Leaf(1), Leaf(2)), Leaf(3)))
";
    let cm = dm(src);
    let f = fid(&cm, "main");
    assert_eq!(eval_core(&cm, f, &[]), Val::I(6));
}

#[test]
fn self_tail_rec_true_for_countdown_loop_helper() {
    let src = "def countdown(n):\n    while n > 0:\n        n = n - 1\n    return n\n";
    let cm = dm(src);
    let helper = helper_named(&cm, "__while");
    assert!(helper.self_tail_rec, "while-loop helper must be self-tail-recursive by construction");
    let f = fid(&cm, "countdown");
    assert_eq!(eval_core(&cm, f, &[Val::I(7)]), Val::I(0));
}

#[test]
fn self_tail_rec_false_for_naive_fib() {
    let src = "\
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
";
    let cm = dm(src);
    let f = fid(&cm, "fib");
    assert!(!cm.fns[f as usize].self_tail_rec, "naive fib's two recursive calls are both inside `+`, not tail calls");
    assert_eq!(eval_core(&cm, f, &[Val::I(10)]), Val::I(55));
}

// ---- if/elif/else -> Core::If ----

#[test]
fn if_else_both_return_desugars_to_plain_core_if() {
    let src = "def pick(c):\n    if c == 1:\n        return 1\n    else:\n        return 0\n";
    let cm = dm(src);
    let f = &cm.fns[fid(&cm, "pick") as usize];
    match &f.body {
        Core::If(_, then, els) => {
            assert_eq!(**then, Core::Num(1));
            assert_eq!(**els, Core::Num(0));
        }
        other => panic!("expected Core::If, got {:?}", other),
    }
    assert_eq!(eval_core(&cm, fid(&cm, "pick"), &[Val::I(1)]), Val::I(1));
    assert_eq!(eval_core(&cm, fid(&cm, "pick"), &[Val::I(0)]), Val::I(0));
}

#[test]
fn if_with_no_else_merges_live_vars_and_falls_through() {
    let src = "def f(n):\n    x = 0\n    if n > 0:\n        x = 1\n    return x\n";
    let cm = dm(src);
    let f = fid(&cm, "f");
    assert_eq!(eval_core(&cm, f, &[Val::I(5)]), Val::I(1));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(0));
}

#[test]
fn elif_chain_desugars_and_evaluates() {
    let src = "\
def sign(n):
    if n > 0:
        return 1
    elif n < 0:
        return 2
    else:
        return 0
";
    let cm = dm(src);
    let f = fid(&cm, "sign");
    assert_eq!(eval_core(&cm, f, &[Val::I(5)]), Val::I(1));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(0));
    assert_eq!(eval_core(&cm, f, &[Val::I(-3)]), Val::I(2));
}

// ---- tuple assignment order / name resolution / arity checking ----

#[test]
fn tuple_literal_and_projection() {
    let src = "def f():\n    t = (10, 20, 30)\n    return t[1]\n";
    let cm = dm(src);
    let f = fid(&cm, "f");
    assert_eq!(eval_core(&cm, f, &[]), Val::I(20));
}

#[test]
fn live_var_merge_order_is_deterministic_across_branches() {
    // Both branches assign a and b in different textual order; the merge
    // must still resolve each name back to the right post-branch value.
    let src = "\
def f(n):
    a = 1
    b = 2
    if n > 0:
        b = 20
        a = 10
    else:
        a = 100
        b = 200
    return a + b
";
    let cm = dm(src);
    let f = fid(&cm, "f");
    assert_eq!(eval_core(&cm, f, &[Val::I(1)]), Val::I(30));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(300));
}

#[test]
fn call_arity_mismatch_is_diag() {
    let src = "def add2(a, b):\n    return a + b\n\ndef caller():\n    return add2(1)\n";
    let d = desugar_err(src);
    assert!(d.msg.contains("add2"), "msg was: {}", d.msg);
}

#[test]
fn ctor_arity_mismatch_is_diag() {
    let src = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def bad():
    return Leaf(1, 2)
";
    let d = desugar_err(src);
    assert!(d.msg.contains("Leaf"), "msg was: {}", d.msg);
}

#[test]
fn unknown_function_call_is_diag() {
    let src = "def f():\n    return nosuchfn(1)\n";
    let d = desugar_err(src);
    assert!(d.msg.contains("nosuchfn"), "msg was: {}", d.msg);
}

// ---- match exhaustiveness on @data types ----

#[test]
fn non_exhaustive_data_match_is_diag_naming_missing_ctor() {
    let src = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def bad(t):
    match t:
        case Leaf(v):
            return v
";
    let d = desugar_err(src);
    assert!(d.msg.contains("Node"), "msg was: {}", d.msg);
}

#[test]
fn exhaustive_data_match_is_accepted() {
    let src = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def val(t):
    match t:
        case Leaf(v):
            return v
        case Node(l, r):
            return 0
";
    dm(src); // must not error
}

#[test]
fn int_literal_match_desugars_to_nested_eq_chain() {
    let src = "\
def classify(n):
    match n:
        case 0:
            return 100
        case 1:
            return 200
";
    let cm = dm(src);
    let f = fid(&cm, "classify");
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(100));
    assert_eq!(eval_core(&cm, f, &[Val::I(1)]), Val::I(200));
}

// ---- "only bool in conditions" ----

#[test]
fn int_in_if_condition_is_diag() {
    let src = "def f(n):\n    if n:\n        return 1\n    return 0\n";
    let d = desugar_err(src);
    assert!(d.msg.to_lowercase().contains("bool"), "msg was: {}", d.msg);
}

#[test]
fn int_in_while_condition_is_diag() {
    let src = "def f(n):\n    while n:\n        n = n - 1\n    return n\n";
    let d = desugar_err(src);
    assert!(d.msg.to_lowercase().contains("bool"), "msg was: {}", d.msg);
}

#[test]
fn cmp_result_stored_in_variable_is_still_boolish_in_a_later_condition() {
    let src = "\
def f(n):
    ok = n > 0
    if ok:
        return 1
    return 0
";
    let cm = dm(src);
    let f = fid(&cm, "f");
    assert_eq!(eval_core(&cm, f, &[Val::I(5)]), Val::I(1));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(0));
}

// ---- and/or/not desugaring ----

#[test]
fn and_or_not_desugar_and_evaluate() {
    let src = "\
def both(a, b):
    return a and b

def either(a, b):
    return a or b

def negate(a):
    return not a
";
    let cm = dm(src);
    assert_eq!(eval_core(&cm, fid(&cm, "both"), &[Val::I(1), Val::I(5)]), Val::I(5));
    assert_eq!(eval_core(&cm, fid(&cm, "both"), &[Val::I(0), Val::I(5)]), Val::I(0));
    assert_eq!(eval_core(&cm, fid(&cm, "either"), &[Val::I(0), Val::I(7)]), Val::I(7));
    assert_eq!(eval_core(&cm, fid(&cm, "either"), &[Val::I(3), Val::I(7)]), Val::I(1));
    assert_eq!(eval_core(&cm, fid(&cm, "negate"), &[Val::I(0)]), Val::I(1));
    assert_eq!(eval_core(&cm, fid(&cm, "negate"), &[Val::I(1)]), Val::I(0));
}

#[test]
fn ternary_ifexp_evaluates_both_arms() {
    let src = "def f(c, a, b):\n    return a if c == 1 else b\n";
    let cm = dm(src);
    let f = fid(&cm, "f");
    assert_eq!(eval_core(&cm, f, &[Val::I(1), Val::I(10), Val::I(20)]), Val::I(10));
    assert_eq!(eval_core(&cm, f, &[Val::I(0), Val::I(10), Val::I(20)]), Val::I(20));
}

// ---- i56 wraparound arithmetic ----

#[test]
fn addition_wraps_at_i56_boundary() {
    let src = "def addone(x):\n    return x + 1\n";
    let cm = dm(src);
    let f = fid(&cm, "addone");
    assert_eq!(eval_core(&cm, f, &[Val::I(I56_MAX)]), Val::I(I56_MIN));
    assert_eq!(eval_core(&cm, f, &[Val::I(0)]), Val::I(1));
}

#[test]
fn floor_div_and_mod_use_python_sign_semantics() {
    let src = "def fd(a, b):\n    return a // b\n\ndef md(a, b):\n    return a % b\n";
    let cm = dm(src);
    assert_eq!(eval_core(&cm, fid(&cm, "fd"), &[Val::I(-7), Val::I(2)]), Val::I(-4));
    assert_eq!(eval_core(&cm, fid(&cm, "md"), &[Val::I(-7), Val::I(2)]), Val::I(1));
    assert_eq!(eval_core(&cm, fid(&cm, "fd"), &[Val::I(7), Val::I(2)]), Val::I(3));
    assert_eq!(eval_core(&cm, fid(&cm, "md"), &[Val::I(7), Val::I(2)]), Val::I(1));
}

// ---- misc plumbing: ctors table, main resolution ----

#[test]
fn ctors_table_records_name_and_arity() {
    let src = "\
@data
class Tree:
    Leaf: (v,)
    Node: (l, r)

def main():
    return 0
";
    let cm = dm(src);
    assert_eq!(cm.ctors, vec![("Leaf".to_string(), 1usize), ("Node".to_string(), 2usize)]);
    let _: CtorId = cm.ctors.len() as u32; // sanity: CtorId is u32-compatible
}

#[test]
fn main_is_resolved_by_name() {
    let src = "def helper():\n    return 1\n\ndef main():\n    return helper() + 1\n";
    let cm = dm(src);
    assert_eq!(cm.fns[cm.main as usize].name, "main");
    assert_eq!(eval_core(&cm, cm.main, &[]), Val::I(2));
}

#[test]
fn single_function_module_defaults_to_that_function_as_main() {
    let src = "def only():\n    return 42\n";
    let cm = dm(src);
    assert_eq!(cm.fns[cm.main as usize].name, "only");
}
