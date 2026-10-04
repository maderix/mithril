//! The Python a first program uses: each feature through the whole front end
//! (lex, parse, infer, desugar) and the reference interpreter, against the
//! value CPython prints; and the programs Mithril rejects, with a message on
//! the right line.

use mithril_front::{desugar, eval_core, parse, Val};

fn run(src: &str) -> Val {
    let m = parse(src).unwrap_or_else(|d| panic!("line {}: {}", d.line, d.msg));
    let cm = desugar(&m).unwrap_or_else(|d| panic!("line {}: {}", d.line, d.msg));
    std::thread::Builder::new().stack_size(1 << 28).spawn(move || eval_core(&cm, cm.main, &[])).unwrap().join().unwrap()
}

fn int(src: &str) -> i64 {
    match run(src) {
        Val::I(v) => v,
        v => panic!("not an int: {v:?}"),
    }
}

/// The diagnostic for `src`: (line, message).
fn rejected(src: &str) -> (u32, String) {
    let d = match parse(src) {
        Err(d) => d,
        Ok(m) => desugar(&m).err().unwrap_or_else(|| panic!("accepted: {src}")),
    };
    (d.line, d.msg)
}

#[test]
fn break_leaves_the_innermost_loop_only() {
    let src = "def main():\n    t = 0\n    for i in range(5):\n        for j in range(5):\n            if j > i:\n                break\n            t = t + 1\n    return t\n";
    assert_eq!(int(src), 15);
    let src = "def main():\n    n = 0\n    while True:\n        n = n + 1\n        if n * n > 50:\n            break\n    return n\n";
    assert_eq!(int(src), 8);
}

#[test]
fn continue_skips_to_the_next_iteration() {
    let src = "def main():\n    t = 0\n    for i in range(10):\n        if i % 2 == 0:\n            continue\n        t = t + i\n    return t\n";
    assert_eq!(int(src), 25);
    // a while loop: the counter update before `continue` must run
    let src = "def main():\n    i = 0\n    t = 0\n    while i < 10:\n        i = i + 1\n        if i % 3 == 0:\n            continue\n        t = t + i\n    return t\n";
    assert_eq!(int(src), 37);
}

#[test]
fn return_inside_loops_leaves_the_function() {
    let src = "def find(n):\n    for i in range(100):\n        if i * i >= n:\n            return i\n    return -1\n\n\ndef main():\n    return find(50) * 1000 + find(100000)\n";
    assert_eq!(int(src), 7999);
    // from a nested loop, and from a while loop
    let src = "def pair(s):\n    for a in range(1, 20):\n        for b in range(a, 20):\n            if a * a + b * b == s:\n                return a * 100 + b\n    return 0\n\n\ndef main():\n    return pair(25) * 10000 + pair(3)\n";
    assert_eq!(int(src), 3040000);
    let src = "def main():\n    n = 27\n    k = 0\n    while n != 1:\n        if k > 1000:\n            return -1\n        n = n // 2 if n % 2 == 0 else 3 * n + 1\n        k = k + 1\n    return k\n";
    assert_eq!(int(src), 111);
    // the value returned from a loop is a tuple
    let src = "def f(n):\n    for i in range(n):\n        if i == 3:\n            return (i, i * i)\n    return (0, 0)\n\n\ndef main():\n    a, b = f(10)\n    c, d = f(2)\n    return a * 1000 + b * 100 + c * 10 + d\n";
    assert_eq!(int(src), 3900);
}

#[test]
fn break_and_continue_outside_a_loop_are_rejected() {
    let (line, msg) = rejected("def main():\n    x = 1\n    break\n");
    assert!(msg.contains("`break` outside a loop") && line == 1, "{line}: {msg}");
}

#[test]
fn augmented_assignment_is_the_operator_and_an_assignment() {
    let src = "def main():\n    x = 100\n    x += 5\n    x -= 3\n    x *= 2\n    x //= 3\n    x %= 50\n    x <<= 2\n    x >>= 1\n    x |= 1\n    x &= 255\n    x ^= 6\n    return x\n";
    assert_eq!(int(src), 35);
}

#[test]
fn chained_comparisons_compare_neighbours() {
    let src = "def main():\n    c = 0\n    for x in range(30):\n        if 2 <= x < 20 != x:\n            c = c + 1\n        if 0 < x < 10 < 2 * x:\n            c = c + 100\n    return c\n";
    assert_eq!(int(src), 418);
}

#[test]
fn powers() {
    assert_eq!(int("def main():\n    return 2 ** 10 + 3 ** 0 + 5 ** 1\n"), 1030);
    // right associative, tighter than unary minus
    assert_eq!(int("def main():\n    return 2 ** 3 ** 2 - (-2 ** 2)\n"), 516);
    // a computed exponent uses the integer helper
    assert_eq!(int("def main():\n    e = 20\n    return 3 ** e\n"), 3486784401);
    // f32 base, literal exponent: repeated multiplication
    assert_eq!(int("def main():\n    x = f32(1.5)\n    return int(x ** 3 * 8.0)\n"), 27);
}

#[test]
fn range_with_a_step() {
    let sum = |args: &str| int(&format!("def main():\n    t = 0\n    for i in range({args}):\n        t = t * 3 + i\n    return t\n"));
    assert_eq!(sum("0, 10, 3"), 54);
    assert_eq!(sum("10, 0, -3"), 346);
    assert_eq!(sum("5, 5, 1"), 0);
    assert_eq!(sum("5, 0, 2"), 0);
    // a step that is only known at run time, both signs
    let src = "def walk(a, b, s):\n    t = 0\n    for i in range(a, b, s):\n        t = t * 3 + i\n    return t\n\n\ndef main():\n    return walk(0, 10, 3) * 10000 + walk(10, 0, -3)\n";
    assert_eq!(int(src), 54 * 10000 + 346);
    let (_, msg) = rejected("def main():\n    t = 0\n    for i in range(0, 5, 0):\n        t = t + i\n    return t\n");
    assert!(msg.contains("step must not be zero"), "{msg}");
}

#[test]
fn min_max_abs_and_a_program_that_defines_its_own() {
    assert_eq!(int("def main():\n    return min(4, 2, 9) * 100 + max(4, 2, 9) * 10 + abs(-3) + abs(3)\n"), 296);
    // on f32 too
    assert_eq!(int("def main():\n    x = f32(-2.5)\n    return int(abs(x) * 2.0) + int(max(x, f32(1.0)))\n"), 6);
    // a function named `max` is the program's, not the builtin
    assert_eq!(int("def max(a, b):\n    return a - b\n\n\ndef main():\n    return max(10, 3)\n"), 7);
}

#[test]
fn module_constants() {
    let src = "N = 10\nSCALE = N * 3\n\n\ndef main():\n    t = 0\n    for i in range(N):\n        t = t + SCALE\n    return t\n";
    assert_eq!(int(src), 300);
    // assigning the name in a function makes it local there (Python)
    let src = "N = 10\n\n\ndef f():\n    N = 4\n    return N\n\n\ndef main():\n    return f() * 100 + N\n";
    assert_eq!(int(src), 410);
}

#[test]
fn docstrings_and_the_main_guard_are_skipped() {
    let src = "\"\"\"A module docstring.\"\"\"\n\n\ndef sq(x):\n    '''Square x.'''\n    return x * x\n\n\ndef main():\n    \"\"\"Entry.\n\n    Spans lines.\"\"\"\n    return sq(7)\n\n\nif __name__ == \"__main__\":\n    print(main())\n";
    assert_eq!(int(src), 49);
}

#[test]
fn pass_is_a_statement_that_does_nothing() {
    assert_eq!(int("def main():\n    x = 3\n    if x > 1:\n        pass\n    else:\n        x = 0\n    return x\n"), 3);
}

#[test]
fn strings_and_item_assignment_are_rejected_clearly() {
    let (line, msg) = rejected("def main():\n    s = \"hello\"\n    return 0\n");
    assert!(msg.contains("strings are not supported") && line == 2, "{line}: {msg}");
    let (line, msg) = rejected("def main():\n    a = array_new(3, 0)\n    a[1] = 5\n    return 0\n");
    assert!(msg.contains("array_set") && line == 3, "{line}: {msg}");
}

#[test]
fn conditions_must_be_numbers_or_comparisons() {
    let (line, msg) = rejected("def main():\n    return 0\n\n\ndef f(t):\n    if (t, t):\n        return 1\n    return 0\n");
    assert!(msg.contains("a condition is a tuple") && line == 5, "{line}: {msg}");
    let (_, msg) = rejected("def main():\n    x = f32(1.0)\n    if x:\n        return 1\n    return 0\n");
    assert!(msg.contains("a condition is an f32"), "{msg}");
    // a function's bool result and a bool argument are conditions
    assert_eq!(int("def even(n):\n    return n % 2 == 0\n\n\ndef pick(flag, x):\n    if flag:\n        return x\n    return 0\n\n\ndef main():\n    return pick(even(4), 7) + pick(even(3), 100)\n"), 7);
}

#[test]
fn integer_true_division_is_rejected_until_floats_convert() {
    let (line, msg) = rejected("def main():\n    a = 7\n    return a / 2\n");
    assert!(msg.contains("`//`") && line == 1, "{line}: {msg}");
}

#[test]
fn errors_after_parsing_name_the_function_and_its_line() {
    let (line, msg) = rejected("def ok():\n    return 1\n\n\ndef main():\n    return missing(3)\n");
    assert!(line == 5 && msg.contains("in 'main'") && msg.contains("missing"), "{line}: {msg}");
}
