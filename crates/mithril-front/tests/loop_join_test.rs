use mithril_front::{desugar, eval_core, parse, Val};

#[test]
fn assigning_the_visible_loop_variable_does_not_advance_the_range() {
    for body in [
        "        total = total + 1\n        i = 99\n",
        "        if i == 1:\n            weight = 2\n        else:\n            weight = 1\n        if weight > 0:\n            total = total + 1\n        i = 99\n",
        "        for i in range(2):\n            total = total + 1\n",
    ] {
        let expected = if body.contains("for i") { 6 } else { 3 };
        let source = format!("def main():\n    total = 0\n    for i in range(3):\n{body}    return total\n");
        let module = desugar(&parse(&source).unwrap()).unwrap();
        assert_eq!(eval_core(&module, module.main, &[]), Val::I(expected));
    }
}

#[test]
fn a_for_continuation_uses_counter_and_bound_in_the_join_scope() {
    let source = "def f(n):\n    total = 0\n    for i in range(n):\n        if i % 2 == 0:\n            weight = 2\n        else:\n            weight = 3\n        if weight > 0:\n            total = total + weight\n    return total\n";
    let module = desugar(&parse(source).unwrap()).unwrap();
    for (n, expected) in [(0, 0), (1, 2), (2, 5), (3, 7), (8, 20)] {
        assert_eq!(
            eval_core(&module, 0, &[Val::I(n)]),
            Val::I(expected),
            "n={n}"
        );
    }
}

#[test]
fn nested_joins_preserve_each_counter_and_evaluate_range_bounds_once() {
    let source = r#"def main():
    lo = 1
    hi = 4
    total = 0
    for i in range(lo, hi):
        if i % 2 == 0:
            weight = 2
        else:
            weight = 3
        for j in range(2):
            if j == 0:
                offset = 1
            else:
                offset = 2
            if offset > 0:
                total = total + i * weight + offset
        lo = 99
        hi = 0
    return total
"#;
    let module = desugar(&parse(source).unwrap()).unwrap();
    assert_eq!(eval_core(&module, module.main, &[]), Val::I(41));
}

#[test]
fn match_joins_preserve_the_for_continuation() {
    let source = r#"@data
class Choice:
    A: ()
    B: ()

def f(n, choice):
    total = 0
    for i in range(n):
        match choice:
            case A():
                weight = 2
            case B():
                weight = 3
        if weight > 0:
            total = total + weight + i
    return total

def main():
    return (f(0, A()), f(3, A()), f(3, B()))
"#;
    let module = desugar(&parse(source).unwrap()).unwrap();
    assert_eq!(
        eval_core(&module, module.main, &[]),
        Val::T(vec![Val::I(0), Val::I(9), Val::I(12)].into())
    );
}
