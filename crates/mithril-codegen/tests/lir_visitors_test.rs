use mithril_codegen::lir::{self, Bop, E, FnDef, Inline, Pat, S, Ty};

#[test]
fn expression_walk_orders_all_control_parts_and_excludes_nested_functions() {
    let read = |n| lir::v(format!("v{n}"));
    let statement = |n| vec![S::Do(read(n))];
    let nested = FnDef { name: "inner".into(), ctx: false, params: vec![], ret: Ty::Unit,
        body: statement(100), inline: Inline::Default, cold: false };
    let body = vec![
        S::Do(E::Call { f: "root".into(), ctx: false, args: vec![read(0), E::Tup(vec![read(1)]),
            lir::bin(Bop::Add, read(2), lir::cast(read(3), Ty::I64))] }),
        S::Loop(statement(4)),
        S::If(read(5), statement(6), statement(7)),
        S::Switch(read(8), vec![(1, statement(9))], Some(statement(10))),
        S::Machine(vec![(0, statement(11))]),
        S::Try(Pat::One("ok".into()), read(12), "err".into(), statement(13)),
        S::Res(read(14), "ok".into(), statement(15), "err".into(), statement(16)),
        S::Fn(Box::new(nested)),
        S::Decl("unused".into(), Ty::I64),
        S::Set("assigned".into(), E::Ref("v17".into())),
        S::Store("out".into(), E::Addr("v18".into())),
        S::Ret(E::Neg(Box::new(E::Not(Box::new(lir::idx(lir::ok(lir::err(read(19))), 0)))))),
    ];
    let mut names = Vec::new();
    lir::walk_exprs(&body, &mut |e| if let E::V(n) | E::Ref(n) | E::Addr(n) = e { names.push(n.clone()); });
    assert_eq!(names, (0..20).map(|n| format!("v{n}")).collect::<Vec<_>>());
    let mut operators = Vec::new();
    lir::walk_exprs(&body, &mut |e| match e {
        E::Call { f, .. } => operators.push(f.clone()), E::Bin(..) => operators.push("bin".into()),
        E::Cast(..) => operators.push("cast".into()), E::Neg(_) => operators.push("neg".into()),
        E::Not(_) => operators.push("not".into()), E::Idx(..) => operators.push("idx".into()),
        E::Ok(_) => operators.push("ok".into()), E::Err(_) => operators.push("err".into()), _ => {}
    });
    assert_eq!(operators, ["root", "bin", "cast", "neg", "not", "idx", "ok", "err"]);
}
