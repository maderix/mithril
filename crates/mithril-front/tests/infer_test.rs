//! The f32 surface (`infer`): elaboration shape, IEEE results against Rust's
//! binary32, inference through every construct, diagnostics, and programs
//! without f32 left unchanged. Also the parser forms it relies on: unary
//! minus, tuple destructuring and bare tuple returns.

use mithril_front::ast::{BinOp, Expr, Stmt};
use mithril_front::{desugar, eval_core, parse, Val};

fn run(src: &str) -> Val {
    let m = parse(src).unwrap_or_else(|d| panic!("parse: {}", d.msg));
    let cm = desugar(&m).unwrap_or_else(|d| panic!("desugar: {}", d.msg));
    eval_core(&cm, cm.main, &[])
}

fn int(src: &str) -> i64 {
    match run(src) {
        Val::I(v) => v,
        v => panic!("expected an int, got {v:?}"),
    }
}

fn err(src: &str) -> String {
    match parse(src) {
        Ok(m) => panic!("expected an error, got {m:?}"),
        Err(d) => d.msg,
    }
}

/// The bit pattern an f32 expression evaluates to (an f32 value is its bit
/// pattern once elaborated).
fn f32_bits(body: &str) -> u32 {
    int(&format!("def main():\n    x = f32(0)\n    x = {body}\n    return x\n")) as u32
}

fn raw(src: &str) -> mithril_front::Module {
    mithril_front::parse::parse_module(&mithril_front::lex::lex(src).unwrap()).unwrap()
}

fn body(m: &mithril_front::Module, f: &str) -> Vec<Stmt> {
    m.fns.iter().find(|x| x.name == f).unwrap().body.clone()
}

// ---- elaboration shape ----

#[test]
fn operators_become_builtins_and_literals_become_bits() {
    let m = parse("def main():\n    x = sqrt(9.0)\n    return x * 2.0 + 1\n").unwrap();
    let Stmt::Return(e) = &body(&m, "main")[1] else { panic!() };
    let two = 2.0f32.to_bits() as i64;
    let one = 1.0f32.to_bits() as i64;
    let x = Expr::Var("x".into());
    let want = Expr::Call("f32_add".into(), vec![Expr::Call("f32_mul".into(), vec![x, Expr::Int(two)]), Expr::Int(one)]);
    assert_eq!(e, &want);
    let nine = Expr::Int(9.0f32.to_bits() as i64);
    assert_eq!(body(&m, "main")[0], Stmt::Assign("x".into(), Expr::Call("f32_sqrt".into(), vec![nine])));
}

#[test]
fn negation_flips_the_sign_bit() {
    let m = parse("def main():\n    x = sqrt(9.0)\n    return -x\n").unwrap();
    let Stmt::Return(e) = &body(&m, "main")[1] else { panic!() };
    assert_eq!(e, &Expr::Bin(BinOp::BitXor, Box::new(Expr::Var("x".into())), Box::new(Expr::Int(1 << 31))));
}

#[test]
fn helpers_are_added_only_when_used() {
    let m = parse("def main():\n    x = sqrt(9.0)\n    return x < 1.0\n").unwrap();
    assert!(m.fns.iter().all(|f| !f.name.starts_with("__")), "no helper needed for <");
    let m = parse("def main():\n    x = sqrt(9.0)\n    return x == 1.0\n").unwrap();
    let names: Vec<_> = m.fns.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["main", "__f32_eq"]);
}

#[test]
fn a_module_without_f32_is_unchanged() {
    for src in [
        "def main():\n    a = 0.5\n    b = a * 2.0\n    return (b, 3 - 1, -a)\n",
        "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\ndef main():\n    x = Cons(1, Nil())\n    y = (x, 2)\n    return y[1] + 1\n",
        "def main():\n    a = array_new(3, 7)\n    return array_get(a, 1) << 2\n",
    ] {
        let mut want = raw(src);
        // raw() skips elaboration: only float negation differs (f64 has no
        // sign-flip rule, so -a is -1.0 * a)
        for f in &mut want.fns {
            for s in &mut f.body {
                if let Stmt::Return(Expr::Tuple(items)) = s {
                    for it in items.iter_mut() {
                        if let Expr::Neg(a) = it {
                            *it = Expr::Bin(BinOp::Mul, Box::new(Expr::Float(-1.0)), a.clone());
                        }
                    }
                }
            }
        }
        assert_eq!(parse(src).unwrap(), want, "{src}");
    }
}

// ---- results against Rust's binary32 ----

#[test]
fn arithmetic_rounds_as_binary32() {
    let cases: [(&str, f32); 6] = [
        ("f32(1) / 3.0", 1.0 / 3.0),
        ("0.1 + 0.2", 0.1f32 + 0.2f32),
        ("f32(16777217)", 16777216.0),
        ("sqrt(2.0) * sqrt(2.0)", 2.0f32.sqrt() * 2.0f32.sqrt()),
        ("-(1.5 - 4.0) / 0.0", f32::INFINITY),
        ("1e-45 * 0.5", 1e-45f32 * 0.5),
    ];
    for (e, want) in cases {
        assert_eq!(f32_bits(e), want.to_bits(), "{e}");
    }
}

#[test]
fn negative_zero_and_nan_follow_ieee() {
    assert_eq!(f32_bits("-f32(0)"), (-0.0f32).to_bits());
    assert_eq!(f32_bits("-0.0"), (-0.0f32).to_bits());
    let r = int("def main():\n    n = sqrt(-1.0)\n    z = f32(0)\n    return ((n < 1.0) + 2 * (n <= 1.0) + 4 * (n > 1.0) + 8 * (n >= 1.0) + 16 * (n == n) + 32 * (n != n) + 64 * (z == -z) + 128 * (z != -z))\n");
    assert_eq!(r, 32 + 64);
}

#[test]
fn comparisons_on_ordinary_values() {
    let r = int("def main():\n    a = f32(2)\n    b = 2.5\n    return (a < b) + 2 * (a <= b) + 4 * (a > b) + 8 * (a >= b) + 16 * (a == b) + 32 * (a != b) + 64 * (a <= 2.0) + 128 * (a >= 2.0)\n");
    assert_eq!(r, 1 + 2 + 32 + 64 + 128);
}

#[test]
fn conversions_truncate_and_round() {
    let src = "def main():\n    return (int(f32(-7) * 0.5), int(-2.75), int(2.75), int(sqrt(-1.0)), int(f32(-3)), int(f32(4000000000)), int(1e20 * f32(1)))\n";
    let want = Val::T(std::sync::Arc::new(vec![Val::I(-3), Val::I(-2), Val::I(2), Val::I(0), Val::I(-3), Val::I(4000000000), Val::I(0)]));
    assert_eq!(run(src), want);
}

// ---- inference through every construct ----

#[test]
fn types_flow_through_calls_returns_tuples_and_destructuring() {
    let src = "
def dot(a, b):
    return a[0] * b[0] + a[1] * b[1]

def half(v):
    return v * 0.5

def main():
    p = (f32(3), 4.0)
    x, y = p
    n = sqrt(dot(p, p))
    return int(half(n) * 10.0) + int(x + y)
";
    assert_eq!(int(src), 25 + 7);
}

#[test]
fn types_flow_through_constructors_and_arrays() {
    let src = "
@data
class H:
    Miss: ()
    At: (t,)

def pick(h):
    match h:
        case Miss():
            return 0.0
        case At(t):
            return t

def main():
    a = array_new(4, 0.0)
    for i in range(4):
        a = array_set(a, i, f32(i) + 0.5)
    s = pick(At(array_get(a, 3))) + pick(Miss())
    return int(s * 2.0)
";
    assert_eq!(int(src), 7);
}

#[test]
fn an_accumulator_seeded_with_a_float_literal_becomes_f32() {
    let src = "def main():\n    s = 0.0\n    for i in range(5):\n        s = s + f32(i) * 0.5\n    return int(s * 10.0)\n";
    assert_eq!(int(src), 50);
    // an int literal is an int: seeding with 0 is a mix
    let e = err("def main():\n    s = 0\n    for i in range(5):\n        s = s + f32(i) * 0.5\n    return int(s)\n");
    assert!(e.contains("used both as f32 and as an int"), "{e}");
}

#[test]
fn a_closure_body_uses_captured_f32() {
    let src = "def main():\n    k = f32(3)\n    g = lambda v: v * 2\n    return int(k * 1.5) + g(4)\n";
    assert_eq!(int(src), 4 + 8);
}

#[test]
fn a_local_named_like_a_builtin_shadows_it_from_its_assignment_on() {
    // before the assignment `sqrt` is the builtin; after it, the closure
    let src = "def main():\n    a = int(sqrt(16.0))\n    sqrt = lambda v: v + 1\n    return a * 10 + sqrt(1)\n";
    assert_eq!(int(src), 42);
}

#[test]
fn a_user_function_shadows_the_builtin_name() {
    let src = "def sqrt(x):\n    return x + 1\n\ndef main():\n    return sqrt(3)\n";
    assert_eq!(int(src), 4);
}

// ---- diagnostics ----

#[test]
fn mixing_an_int_variable_with_f32_is_an_error() {
    let e = err("def main():\n    x = f32(1)\n    for i in range(3):\n        x = x * i\n    return 0\n");
    assert!(e.contains("used both as f32 and as an int"), "{e}");
    assert!(e.contains("'main'"), "{e}");
}

#[test]
fn integer_operators_on_f32_are_errors() {
    for op in ["%", "//", "<<", ">>", "&", "|", "^"] {
        let e = err(&format!("def main():\n    x = f32(1)\n    return x {op} 2.0\n"));
        assert!(e.contains("not defined on f32"), "{op}: {e}");
    }
}

#[test]
fn f32_as_a_tuple_or_constructor_is_an_error() {
    let e = err("def main():\n    x = f32(1)\n    x = (1, 2)\n    return 0\n");
    assert!(e.contains("as a tuple"), "{e}");
}

#[test]
fn converting_a_float_variable_makes_it_f32() {
    assert_eq!(int("def main():\n    a = 0.5\n    b = a * 7.0\n    return int(b)\n"), 3);
    let e = err("def main():\n    a = 0.5\n    a = 1\n    return int(a)\n");
    assert!(e.contains("used both as an int and as a float"), "{e}");
}

// ---- one type per function, field and slot (review of 4b01016) ----

#[test]
fn a_helper_used_at_int_and_f32_is_an_error_not_a_retyping() {
    for src in [
        "def sq(x):\n    return x * x\n\ndef main():\n    a = sq(3)\n    b = sq(sqrt(2.0))\n    return a + 1\n",
        "@data\nclass L:\n    Nil: ()\n    Cons: (h, t)\n\ndef main():\n    x = Cons(16777217, Nil())\n    y = Cons(sqrt(2.0), Nil())\n    return 0\n",
        "def f(p):\n    return p[0]\n\ndef main():\n    return f((5, 1)) + int(f((sqrt(2.0), 1)))\n",
    ] {
        let e = err(src);
        assert!(e.contains("used both as f32 and as an int"), "{src}: {e}");
    }
}

#[test]
fn f32_meeting_a_mixed_value_is_an_error() {
    let src = "def id(v):\n    return v\n\ndef main():\n    x = id((1, 2))\n    w = id(3 < 4)\n    y = id(sqrt(f32(4)))\n    return 0\n";
    let e = err(src);
    assert!(e.contains("f32 and as"), "{e}");
}

#[test]
fn and_or_results_are_ints() {
    let e = err("def main():\n    x = sqrt(2.0)\n    c = 1 < 2\n    return (c and x) + x\n");
    assert!(e.contains("used both as f32 and as an int"), "{e}");
}

#[test]
fn conversions_cover_the_whole_int_range() {
    // f32(n) rounds correctly for every i56; int(x) is exact below 2^55
    for n in [5_000_000_000i64, -5_000_000_000, (1 << 55) - 1, -(1 << 55) + 1, 16_777_217, (1 << 40) + (1 << 16) + 1] {
        assert_eq!(f32_bits(&format!("f32({n})")), (n as f32).to_bits(), "f32({n})");
        let src = format!("def main():\n    n = {n}\n    return f32(n)\n");
        assert_eq!(int(&src) as u32, (n as f32).to_bits(), "f32 of a variable {n}");
    }
    for x in ["5000000000.0", "-5000000000.0", "3.0e16", "2.5e16 * f32(1)"] {
        let v: f32 = x.split(' ').next().unwrap().parse::<f32>().unwrap();
        assert_eq!(int(&format!("def main():\n    return int({x})\n")), v as i64, "int({x})");
    }
    assert_eq!(int("def main():\n    return int(1e20 * f32(1))\n"), 0, "past 2^55");
    // the smallest i56 (no literal spells it)
    let min = "def main():\n    n = 0 - 36028797018963967 - 1\n    return f32(n)\n";
    assert_eq!(int(min) as u32, (-(1i64 << 55) as f32).to_bits());
}

#[test]
fn negation_follows_the_value() {
    assert_eq!(run("def main():\n    a = 1.5\n    return -a\n"), Val::F(-1.5));
    assert_eq!(int("def main():\n    a = 7\n    return -a\n"), -7);
    let e = err("def neg(x):\n    return -x\n\ndef main():\n    a = neg(3)\n    b = neg(1.5)\n    return a\n");
    assert!(e.contains("negation of a value used both as an int and as an f64"), "{e}");
}

#[test]
fn unpacking_checks_the_count() {
    for src in ["def main():\n    a, b, c = (1, 2)\n    return a\n", "def main():\n    x, y = (5, 6, 7)\n    return x\n", "def main():\n    x, y = 5\n    return x\n"] {
        let e = err(src);
        assert!(e.contains("cannot unpack a value into"), "{src}: {e}");
    }
    assert_eq!(int("def two():\n    return 3, 4\n\ndef main():\n    x, y = two()\n    return x * 10 + y\n"), 34);
}

// ---- parser forms ----

#[test]
fn unary_minus_precedence_and_folding() {
    let cases = [("-2 * 3 + 7", 1), ("2 - -3", 5), ("-(2 + 3) * 2", -10), ("-f(2)", -4), ("-(1, 9)[1] + 10", 1), ("--4", 4)];
    for (e, want) in cases {
        assert_eq!(int(&format!("def f(x):\n    return x * 2\n\ndef main():\n    return {e}\n")), want, "{e}");
    }
    let m = raw("def main():\n    return -3\n");
    assert_eq!(body(&m, "main")[0], Stmt::Return(Expr::Int(-3)));
    assert!(err("def main():\n    return +3\n").contains("unary plus"));
}

#[test]
fn exponent_literals() {
    assert_eq!(f32_bits("1e3 + 2.5E-2"), (1e3f32 + 2.5e-2f32).to_bits());
    assert_eq!(f32_bits("f32(2) * 4e+1"), 80.0f32.to_bits());
    // `e` without digits is a name
    assert!(err("def main():\n    return 1e\n").contains("expected end of statement"));
}

#[test]
fn tuple_destructuring_and_bare_tuples() {
    assert_eq!(int("def main():\n    a, b = 1, 2\n    a, b = b, a\n    return a * 10 + b\n"), 21);
    assert_eq!(run("def two():\n    return 3, 4\n\ndef main():\n    x, y = two()\n    return y, x\n"), Val::T(std::sync::Arc::new(vec![Val::I(4), Val::I(3)])));
    assert!(err("def main():\n    a, 1 = 1, 2\n    return a\n").contains("invalid assignment target"));
}
