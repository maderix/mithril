use super::*;
use mithril_front::ast::BinOp;
fn call(args: Vec<Core>) -> Core { Core::Call(u32::MAX, args) }
fn bind(x: u32, rhs: Core, body: Core) -> Core { Core::Let(x, Box::new(rhs), Box::new(body)) }

#[test]
fn normalization_preserves_hoist_order_branch_scopes_and_constructor_binders() {
    let lambda = Core::Lam(42, Box::new(call(vec![Core::Var(42)])));
    let matched = Core::Match(Box::new(call(vec![])), vec![(0, vec![5], call(vec![Core::Var(5)])), (1, vec![6], Core::Num(0))]);
    let branch = Core::If(Box::new(call(vec![])), Box::new(call(vec![Core::Var(1)])), Box::new(matched));
    let expression = Core::Tuple(vec![Core::Op2(BinOp::Add, Box::new(call(vec![call(vec![Core::Var(0)])])), Box::new(Core::Num(1))), branch, lambda.clone()]);
    let expected_match = bind(103, call(vec![]), Core::Match(Box::new(Core::Var(103)), vec![(0, vec![5], call(vec![Core::Var(5)])), (1, vec![6], Core::Num(0))]));
    let expected_branch = Core::If(Box::new(Core::Var(102)), Box::new(call(vec![Core::Var(1)])), Box::new(expected_match));
    let expected = bind(100, call(vec![Core::Var(0)]), bind(101, call(vec![Core::Var(100)]), bind(102, call(vec![]), bind(104, expected_branch,
        Core::Tuple(vec![Core::Op2(BinOp::Add, Box::new(Core::Var(101)), Box::new(Core::Num(1))), Core::Var(104), lambda])))));
    let mut next = 100;
    assert_eq!(normalize(&expression, &mut next), expected);
    assert_eq!(next, 105, "fresh identifiers include only actual hoists");
}

#[test]
fn call_free_trees_and_unexecuted_lambda_bodies_do_not_mint_bindings() {
    for expression in [Core::Tuple(vec![Core::Num(1), Core::Var(9)]), Core::Lam(0, Box::new(call(vec![Core::Var(0)])))] {
        let mut next = 100;
        assert_eq!(normalize(&expression, &mut next), expression);
        assert_eq!(next, 100);
    }
}
