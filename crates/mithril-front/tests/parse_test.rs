use mithril_front::ast::*;
use mithril_front::parse;

fn ok(src: &str) -> Module {
    match parse(src) {
        Ok(m) => m,
        Err(d) => panic!("expected parse success, got Diag at line {}: {}", d.line, d.msg),
    }
}

fn err(src: &str) -> mithril_front::Diag {
    match parse(src) {
        Ok(m) => panic!("expected parse failure, got: {:?}", m),
        Err(d) => d,
    }
}

// ---- literals, names, simple statements ----

#[test]
fn assign_and_return_int_literal() {
    let m = ok("def f():\n    x = 1\n    return x\n");
    assert_eq!(
        m.fns,
        vec![FnDef {
            name: "f".into(),
            params: vec![],
            body: vec![
                Stmt::Assign("x".into(), Expr::Int(1)),
                Stmt::Return(Expr::Var("x".into())),
            ],
        }]
    );
}

#[test]
fn float_and_bool_literals() {
    let m = ok("def f():\n    x = 1.5\n    y = True\n    z = False\n    return x\n");
    assert_eq!(
        m.fns[0].body,
        vec![
            Stmt::Assign("x".into(), Expr::Float(1.5)),
            Stmt::Assign("y".into(), Expr::Bool(true)),
            Stmt::Assign("z".into(), Expr::Bool(false)),
            Stmt::Return(Expr::Var("x".into())),
        ]
    );
}

#[test]
fn hex_int_literal() {
    let m = ok("def f():\n    x = 0x1F\n    return x\n");
    assert_eq!(m.fns[0].body[0], Stmt::Assign("x".into(), Expr::Int(31)));
}

#[test]
fn expr_stmt() {
    let m = ok("def f():\n    f(1)\n    return 0\n");
    assert_eq!(m.fns[0].body[0], Stmt::ExprStmt(Expr::Call("f".into(), vec![Expr::Int(1)])));
}

#[test]
fn params_and_call() {
    let m = ok("def add(a, b):\n    return a + b\n\ndef main():\n    return add(1, 2)\n");
    assert_eq!(m.fns[0].params, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(
        m.fns[1].body[0],
        Stmt::Return(Expr::Call("add".into(), vec![Expr::Int(1), Expr::Int(2)]))
    );
}

// ---- binary ops, precedence ----

#[test]
fn binary_ops_all_kinds() {
    let cases: Vec<(&str, BinOp)> = vec![
        ("a + b", BinOp::Add),
        ("a - b", BinOp::Sub),
        ("a * b", BinOp::Mul),
        ("a / b", BinOp::Div),
        ("a // b", BinOp::FloorDiv),
        ("a % b", BinOp::Mod),
        ("a << b", BinOp::Shl),
        ("a >> b", BinOp::Shr),
        ("a & b", BinOp::BitAnd),
        ("a | b", BinOp::BitOr),
        ("a ^ b", BinOp::BitXor),
    ];
    for (src, op) in cases {
        let m = ok(&format!("def f(a, b):\n    return {}\n", src));
        assert_eq!(
            m.fns[0].body[0],
            Stmt::Return(Expr::Bin(op, Box::new(Expr::Var("a".into())), Box::new(Expr::Var("b".into())))),
            "mismatch for {src}"
        );
    }
}

#[test]
fn cmp_ops_all_kinds() {
    let cases: Vec<(&str, CmpOp)> = vec![
        ("a < b", CmpOp::Lt),
        ("a <= b", CmpOp::Le),
        ("a > b", CmpOp::Gt),
        ("a >= b", CmpOp::Ge),
        ("a == b", CmpOp::Eq),
        ("a != b", CmpOp::Ne),
    ];
    for (src, op) in cases {
        let m = ok(&format!("def f(a, b):\n    return {}\n", src));
        assert_eq!(
            m.fns[0].body[0],
            Stmt::Return(Expr::Cmp(op, Box::new(Expr::Var("a".into())), Box::new(Expr::Var("b".into())))),
            "mismatch for {src}"
        );
    }
}

#[test]
fn bool_ops_and_not() {
    let m = ok("def f(a, b):\n    return a and b or not a\n");
    // 'or' is lowest, 'and' binds tighter, 'not' tighter still.
    let expected = Expr::Bool2(
        BoolOp::Or,
        Box::new(Expr::Bool2(BoolOp::And, Box::new(Expr::Var("a".into())), Box::new(Expr::Var("b".into())))),
        Box::new(Expr::Not(Box::new(Expr::Var("a".into())))),
    );
    assert_eq!(m.fns[0].body[0], Stmt::Return(expected));
}

#[test]
fn precedence_mul_before_add() {
    let m = ok("def f(a, b, c):\n    return a + b * c\n");
    let expected = Expr::Bin(
        BinOp::Add,
        Box::new(Expr::Var("a".into())),
        Box::new(Expr::Bin(BinOp::Mul, Box::new(Expr::Var("b".into())), Box::new(Expr::Var("c".into())))),
    );
    assert_eq!(m.fns[0].body[0], Stmt::Return(expected));
}

#[test]
fn precedence_add_before_shift_before_bitand_before_bitxor_before_bitor() {
    let m = ok("def f(a, b, c, d, e):\n    return a | b ^ c & d << e\n");
    // bitor(bitxor(b, bitand(c, shift(d,e))))... actually: a | (b ^ (c & (d << e)))
    let expected = Expr::Bin(
        BinOp::BitOr,
        Box::new(Expr::Var("a".into())),
        Box::new(Expr::Bin(
            BinOp::BitXor,
            Box::new(Expr::Var("b".into())),
            Box::new(Expr::Bin(
                BinOp::BitAnd,
                Box::new(Expr::Var("c".into())),
                Box::new(Expr::Bin(BinOp::Shl, Box::new(Expr::Var("d".into())), Box::new(Expr::Var("e".into())))),
            )),
        )),
    );
    assert_eq!(m.fns[0].body[0], Stmt::Return(expected));
}

#[test]
fn left_associative_add_sub() {
    let m = ok("def f(a, b, c):\n    return a - b - c\n");
    let expected = Expr::Bin(
        BinOp::Sub,
        Box::new(Expr::Bin(BinOp::Sub, Box::new(Expr::Var("a".into())), Box::new(Expr::Var("b".into())))),
        Box::new(Expr::Var("c".into())),
    );
    assert_eq!(m.fns[0].body[0], Stmt::Return(expected));
}

#[test]
fn parens_override_precedence() {
    let m = ok("def f(a, b, c):\n    return (a + b) * c\n");
    let expected = Expr::Bin(
        BinOp::Mul,
        Box::new(Expr::Bin(BinOp::Add, Box::new(Expr::Var("a".into())), Box::new(Expr::Var("b".into())))),
        Box::new(Expr::Var("c".into())),
    );
    assert_eq!(m.fns[0].body[0], Stmt::Return(expected));
}

// ---- ternary, tuple, index, lambda, calls ----

#[test]
fn ternary_ifexp() {
    let m = ok("def f(a, b, c):\n    return a if c else b\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::Return(Expr::IfExp(
            Box::new(Expr::Var("c".into())),
            Box::new(Expr::Var("a".into())),
            Box::new(Expr::Var("b".into())),
        ))
    );
}

#[test]
fn tuple_literal_and_singleton() {
    let m = ok("def f(a, b):\n    x = (a, b)\n    y = (a,)\n    z = ()\n    return x\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::Assign("x".into(), Expr::Tuple(vec![Expr::Var("a".into()), Expr::Var("b".into())]))
    );
    assert_eq!(m.fns[0].body[1], Stmt::Assign("y".into(), Expr::Tuple(vec![Expr::Var("a".into())])));
    assert_eq!(m.fns[0].body[2], Stmt::Assign("z".into(), Expr::Tuple(vec![])));
}

#[test]
fn parenthesized_single_expr_is_not_a_tuple() {
    let m = ok("def f(a):\n    return (a)\n");
    assert_eq!(m.fns[0].body[0], Stmt::Return(Expr::Var("a".into())));
}

#[test]
fn indexing() {
    let m = ok("def f(t):\n    return t[0]\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::Return(Expr::Index(Box::new(Expr::Var("t".into())), Box::new(Expr::Int(0))))
    );
}

#[test]
fn chained_indexing() {
    let m = ok("def f(t):\n    return t[0][1]\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::Return(Expr::Index(
            Box::new(Expr::Index(Box::new(Expr::Var("t".into())), Box::new(Expr::Int(0)))),
            Box::new(Expr::Int(1)),
        ))
    );
}

#[test]
fn lambda_expr() {
    let m = ok("def f():\n    g = lambda a, b: a + b\n    return g\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::Assign(
            "g".into(),
            Expr::Lambda(
                vec!["a".into(), "b".into()],
                Box::new(Expr::Bin(BinOp::Add, Box::new(Expr::Var("a".into())), Box::new(Expr::Var("b".into())))),
            )
        )
    );
}

// ---- if/elif/else, while, for, match ----

#[test]
fn if_else() {
    let m = ok("def f(a):\n    if a:\n        return 1\n    else:\n        return 2\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::If(Expr::Var("a".into()), vec![Stmt::Return(Expr::Int(1))], vec![Stmt::Return(Expr::Int(2))])
    );
}

#[test]
fn if_no_else() {
    let m = ok("def f(a):\n    if a:\n        return 1\n    return 2\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::If(Expr::Var("a".into()), vec![Stmt::Return(Expr::Int(1))], vec![])
    );
    assert_eq!(m.fns[0].body[1], Stmt::Return(Expr::Int(2)));
}

#[test]
fn if_elif_else_chain() {
    let m = ok(
        "def f(a, b):\n    if a:\n        return 1\n    elif b:\n        return 2\n    else:\n        return 3\n",
    );
    let expected = Stmt::If(
        Expr::Var("a".into()),
        vec![Stmt::Return(Expr::Int(1))],
        vec![Stmt::If(
            Expr::Var("b".into()),
            vec![Stmt::Return(Expr::Int(2))],
            vec![Stmt::Return(Expr::Int(3))],
        )],
    );
    assert_eq!(m.fns[0].body[0], expected);
}

#[test]
fn while_loop() {
    let m = ok("def f(n):\n    while n:\n        n = n - 1\n    return n\n");
    assert_eq!(
        m.fns[0].body[0],
        Stmt::While(
            Expr::Var("n".into()),
            vec![Stmt::Assign(
                "n".into(),
                Expr::Bin(BinOp::Sub, Box::new(Expr::Var("n".into())), Box::new(Expr::Int(1)))
            )]
        )
    );
}

#[test]
fn for_range_loop() {
    let m = ok("def f(n):\n    s = 0\n    for i in range(n):\n        s = s + i\n    return s\n");
    assert_eq!(
        m.fns[0].body[1],
        Stmt::For(
            "i".into(),
            Expr::Var("n".into()),
            vec![Stmt::Assign(
                "s".into(),
                Expr::Bin(BinOp::Add, Box::new(Expr::Var("s".into())), Box::new(Expr::Var("i".into())))
            )],
            None,
        )
    );
}

#[test]
fn match_ctor_and_int_patterns() {
    let m = ok(
        "def f(x):\n    match x:\n        case Leaf(v):\n            return v\n        case 0:\n            return 0\n",
    );
    assert_eq!(
        m.fns[0].body[0],
        Stmt::Match(
            Expr::Var("x".into()),
            vec![
                (Pat { ctor: "Leaf".into(), binds: vec!["v".into()] }, vec![Stmt::Return(Expr::Var("v".into()))]),
                (Pat::int_lit(0), vec![Stmt::Return(Expr::Int(0))]),
            ]
        )
    );
}

// ---- @data decl ----

#[test]
fn data_decl() {
    let m = ok("@data\nclass Tree:\n    Leaf: (v,)\n    Node: (l, r)\n\ndef main():\n    return 0\n");
    assert_eq!(
        m.datas,
        vec![DataDef {
            name: "Tree".into(),
            ctors: vec![
                ("Leaf".into(), vec!["v".into()]),
                ("Node".into(), vec!["l".into(), "r".into()]),
            ],
        }]
    );
}

// ---- errors ----

#[test]
fn tab_indentation_is_diag() {
    let d = err("def f():\n\treturn 1\n");
    assert!(d.msg.to_lowercase().contains("tab"), "msg was: {}", d.msg);
}

#[test]
fn import_statement_is_diag_naming_import() {
    let d = err("def f():\n    import x\n    return 1\n");
    assert!(d.msg.contains("import"), "msg was: {}", d.msg);
}

#[test]
fn int_literal_out_of_range_is_diag() {
    // 2**70 written as a decimal literal, out of i64 range.
    let d = err("def f():\n    x = 1180591620717411303424\n    return x\n");
    assert_eq!(d.msg, "int literal out of range");
}

#[test]
fn i64_max_literal_is_ok_but_one_more_overflows() {
    ok("def f():\n    x = 36028797018963968\n    return x\n"); // 2**55: past the inline form, still an int
    ok("def f():\n    x = 9223372036854775807\n    return x\n"); // 2**63 - 1
    let d = err("def f():\n    x = 9223372036854775808\n    return x\n"); // 2**63
    assert_eq!(d.msg, "int literal out of range");
}

#[test]
fn mismatched_dedent_is_diag_with_line() {
    let d = err("def f():\n    x = 1\n  y = 2\n");
    assert_eq!(d.line, 3);
}

#[test]
fn unknown_construct_still_reports_a_line() {
    let d = err("def f():\n    with x:\n        return 1\n");
    assert!(d.line == 2);
}

#[test]
fn hex_literal_past_the_i64_range_is_rejected_not_wrapped() {
    assert!(err("def f():\n    return 0xFFFFFFFFFFFFFFFF\n").msg.contains("out of range"));
    assert!(err("def f():\n    return 0x8000000000000000\n").msg.contains("out of range"));
    let m = ok("def f():\n    return 0x7FFFFFFFFFFFFFFF\n");
    assert_eq!(m.fns[0].body[0], Stmt::Return(Expr::Int(0x7FFFFFFFFFFFFFFF)));
    let m = ok("def f():\n    return 0x80000000000000\n");
    assert_eq!(m.fns[0].body[0], Stmt::Return(Expr::Int(0x80000000000000)));
}
