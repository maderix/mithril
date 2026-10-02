use mithril_front::ast::{BinOp, BoolOp, CmpOp, Expr, Pat, Stmt};
use mithril_front::desugar::{free_reads_expr, free_reads_stmts};
use std::collections::BTreeSet;

fn var(name: &str) -> Expr { Expr::Var(name.into()) }

#[test]
fn expression_traversal_preserves_order_scope_and_short_circuiting() {
    let expression = Expr::IfExp(
        Box::new(Expr::Cmp(CmpOp::Eq, Box::new(var("condition")), Box::new(Expr::Int(0)))),
        Box::new(Expr::Tuple(vec![
            Expr::Bin(BinOp::Add, Box::new(var("a")), Box::new(Expr::Float(1.0))),
            Expr::Bool2(BoolOp::And, Box::new(Expr::Bool(true)), Box::new(Expr::Not(Box::new(var("b"))))),
            Expr::Neg(Box::new(Expr::Index(Box::new(var("tuple")), Box::new(Expr::Int(1))))),
            Expr::Lambda(vec!["bound".into()], Box::new(Expr::Call("callee".into(), vec![var("bound"), var("capture")]))),
        ])),
        Box::new(var("otherwise")),
    );
    let names = expression.fold(&mut |node, mut children: Vec<Vec<String>>| {
        let mut names: Vec<_> = children.drain(..).flatten().collect();
        if let Expr::Var(name) = node { names.push(name.clone()); }
        names
    });
    assert_eq!(names, ["condition", "a", "b", "tuple", "bound", "capture", "otherwise"]);
    let mut reads = BTreeSet::new();
    free_reads_expr(&expression, &mut reads);
    assert_eq!(reads, ["condition", "a", "b", "tuple", "callee", "capture", "otherwise"].into_iter().map(str::to_string).collect());
    assert!(expression.any(&|node| {
        assert!(std::ptr::eq(node, &expression), "short-circuit search descended after a hit");
        true
    }));
    assert!(!expression.any(&|node| matches!(node, Expr::Var(name) if name == "missing")));
}

#[test]
fn statement_traversal_retains_every_head_block_and_binder() {
    let statements = vec![
        Stmt::Assign("assigned".into(), var("assignment")),
        Stmt::Return(var("return")),
        Stmt::ExprStmt(var("effect")),
        Stmt::If(var("if"), vec![Stmt::Return(var("then"))], vec![Stmt::Return(var("else"))]),
        Stmt::While(var("while"), vec![Stmt::Return(var("while_body"))]),
        Stmt::For("index".into(), var("bound"), vec![Stmt::Return(var("for_body"))], None),
        Stmt::Match(var("scrutinee"), vec![
            (Pat { ctor: "Some".into(), binds: vec!["pattern".into()] }, vec![Stmt::Return(var("pattern"))]),
            (Pat::int_lit(0), Vec::new()),
        ]),
    ];
    let heads: Vec<_> = statements.iter().map(|st| st.parts().0.clone()).collect();
    assert_eq!(heads, ["assignment", "return", "effect", "if", "while", "bound", "scrutinee"].map(var));
    assert_eq!(statements.iter().map(|st| st.parts().1.len()).collect::<Vec<_>>(), [0, 0, 0, 2, 1, 1, 2]);
    let mut reads = BTreeSet::new();
    free_reads_stmts(&statements, &mut reads);
    assert_eq!(reads.len(), 12);
    assert!(reads.contains("pattern"));
    assert!(!reads.contains("assigned") && !reads.contains("index"));
    if let Stmt::Match(_, arms) = &statements[6] { assert_eq!(arms[0].0.binds, ["pattern"]); }
}
