use super::*;

#[test]
fn immediate_and_deferred_projections_share_component_and_poison_resolution() {
    let mut inf = Inf::default();
    let int = inf.int();
    let float = inf.node(Node::Flo);
    inf.tups.push(vec![int, float]);
    let tuple = inf.node(Node::Tup(2, 0));
    assert_eq!(inf.proj(tuple, 0), int);
    assert_eq!(inf.proj(tuple, 1), float);
    let base = inf.uf.fresh();
    let delayed = inf.proj(base, 1);
    let invalid = inf.proj(base, 2);
    inf.unify(base, tuple);
    inf.settle();
    assert_eq!(inf.uf.read(delayed), Ty::Flo);
    assert_eq!(inf.uf.read(invalid), Ty::Dyn);
    assert!(inf.pending.is_empty());
    let unknown = inf.uf.fresh();
    let delayed = inf.proj(unknown, 0);
    inf.set(unknown, Node::Adt(u32::MAX));
    inf.settle();
    assert_eq!(inf.uf.read(delayed), Ty::Dyn);
    let immediate = inf.proj(unknown, 0);
    let root = inf.uf.find(immediate);
    assert!(inf.uf.n[root as usize] == Node::Adt(u32::MAX));
    let array = inf.arr_of(int);
    let read = inf.proj(array, 0);
    inf.settle();
    assert_eq!(inf.uf.read(read), Ty::Dyn, "arrays must not become tuple projections");
}

#[test]
fn conflicting_recursive_shapes_poison_descendants_and_terminate() {
    let mut inf = Inf::default();
    let int = inf.int();
    let recursive = inf.uf.fresh();
    inf.tups.push(vec![int, recursive]);
    inf.set(recursive, Node::Tup(2, 0));
    assert!(inf.shape(recursive).is_none(), "cyclic types have no native layout");
    let array = inf.arr_of(recursive);
    inf.set(array, Node::Flo);
    assert_eq!(inf.uf.read(array), Ty::Dyn);
    assert_eq!(inf.uf.read(recursive), Ty::Dyn);
    assert_eq!(inf.uf.read(int), Ty::Dyn);
}

#[test]
fn minted_result_conflicts_stay_local_until_the_operand_kind_changes() {
    let mut inf = Inf::default();
    let parameter = inf.uf.fresh();
    let mut env = vec![parameter];
    let expression = Core::Op2(mithril_front::ast::BinOp::Add, Box::new(Core::Var(0)), Box::new(Core::Num(1)));
    let result = inf.walk(0, &expression, &mut env);
    inf.set(result, Node::Flo);
    inf.relink();
    assert_eq!(inf.uf.read(parameter), Ty::Int);
    assert_eq!(inf.uf.read(result), Ty::Dyn);
    inf.set(parameter, Node::Flo);
    inf.relink();
    assert_eq!(inf.uf.read(parameter), Ty::Dyn);
    assert_eq!(inf.uf.read(result), Ty::Dyn);
}
