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

#[test]
fn control_flow_summaries_preserve_fallthrough_bindings_and_loop_state() {
    for (name, source, expected) in [
        ("returning_arms", r#"def f(n):
    if n < 0:
        return 7
    else:
        x = n + 1
    if x > 2:
        return x * 2
    return x + 10

def main():
    return (f(0), f(3), f(0 - 1))
"#, Val::T(vec![Val::I(11), Val::I(8), Val::I(7)].into())),
        ("pattern_escape", r#"@data
class Choice:
    A: (x,)
    B: (x,)

def f(t):
    match t:
        case A(value):
            other = value + 1
        case B(value):
            other = value + 2
    if value > 0:
        other = other + value
    return other

def main():
    return (f(A(3)), f(B(4)))
"#, Val::T(vec![Val::I(7), Val::I(10)].into())),
        ("returning_pattern", r#"@data
class Choice:
    A: (x,)
    B: (x,)

def f(t):
    match t:
        case A(value):
            return value
        case B(value):
            result = value + 1
    if result > 0:
        result = result + value
    return result

def main():
    return (f(A(2)), f(B(3)))
"#, Val::T(vec![Val::I(2), Val::I(7)].into())),
        ("integer_fallthrough", r#"def f(n):
    total = 3
    match n:
        case 0:
            total = 9
        case 1:
            return 11
    if total > 0:
        total = total + n
    return total

def main():
    return (f(0), f(1), f(5))
"#, Val::T(vec![Val::I(9), Val::I(11), Val::I(8)].into())),
        ("zero_trip_state", r#"def f(n):
    total = 4
    done = False
    for i in range(n):
        total = total + i
        done = i == 2
    while total < 4:
        total = total + 1
    if done:
        return total + 100
    return total

def main():
    return (f(0), f(0 - 2), f(3))
"#, Val::T(vec![Val::I(4), Val::I(4), Val::I(107)].into())),
        ("shadowed_closure", r#"def add(n):
    return n + 100

def f(n):
    add = lambda x: x + 1
    total = 0
    for i in range(n):
        if i == 1:
            weight = 2
        else:
            weight = 1
        while weight > 0:
            total = total + add(i)
            weight = weight - 1
    return total

def main():
    return (f(0), f(3))
"#, Val::T(vec![Val::I(0), Val::I(8)].into())),
        ("empty_loop_state", r#"def main():
    for i in range(0):
        local = i + 1
    while False:
        local = 3
    return 9
"#, Val::I(9)),
        ("fold_metadata", r#"def f(n):
    total = 0
    for i in range(n):
        total = total + i
    return total

def main():
    return f(5)
"#, Val::I(10)),
        ("f32_shadow", r#"def f32(n):
    return n + 2

def main():
    total = 0
    for i in range(3):
        total = total + f32(i)
    return total
"#, Val::I(9)),
    ] {
        let module = desugar(&parse(source).unwrap()).unwrap();
        assert_eq!(eval_core(&module, module.main, &[]), expected, "{name}");
    }
}

#[test]
fn control_flow_summaries_preserve_diagnostics() {
    for (name, source, message) in [
        ("last_statement", "def main():\n    return 3\n    x = 4\n", "does not return on all control-flow paths"),
        ("zero_trip_binding", "def main():\n    for i in range(0):\n        x = 3\n    return x\n", "unknown variable: x"),
        ("int_match_binding", "def f(n):\n    match n:\n        case 0:\n            x = 1\n    if n > 0:\n        return x\n    return 2\n", "unknown variable: x"),
    ] {
        let error = desugar(&parse(source).unwrap()).unwrap_err();
        assert!(error.msg.contains(message), "{name}: {}", error.msg);
    }
    // once rejected, now Python's meaning: a return inside loops, an int as a condition
    for (name, source, expected) in [
        ("nested_while_return", "def main():\n    while False:\n        if True:\n            return 1\n    return 2\n", 2),
        ("nested_for_return", "def main():\n    for i in range(0):\n        while False:\n            return 1\n    return 2\n", 2),
        ("bool_reassignment", "def main():\n    ok = True\n    for i in range(2):\n        ok = i\n    if ok:\n        return 1\n    return 2\n", 1),
    ] {
        let module = desugar(&parse(source).unwrap()).unwrap();
        assert_eq!(eval_core(&module, module.main, &[]), Val::I(expected), "{name}");
    }
}
