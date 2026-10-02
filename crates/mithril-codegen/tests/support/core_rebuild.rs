use super::*;
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::Prim;

#[test]
fn child_rebuild_preserves_payloads_binders_and_left_to_right_order() {
    let nodes = vec![Core::Num(7), Core::Flo(1.5), Core::Var(8),
        Core::Op2(BinOp::Sub, Box::new(Core::Var(0)), Box::new(Core::Var(1))),
        Core::Cmp(CmpOp::Le, Box::new(Core::Var(0)), Box::new(Core::Var(1))),
        Core::Let(99, Box::new(Core::Var(0)), Box::new(Core::Var(1))),
        Core::App(Box::new(Core::Var(0)), Box::new(Core::Var(1))),
        Core::If(Box::new(Core::Var(0)), Box::new(Core::Var(1)), Box::new(Core::Var(2))),
        Core::Call(77,vec![Core::Var(0),Core::Var(1)]), Core::Ctor(88,vec![Core::Var(0)]),
        Core::Reuse(99,88,vec![Core::Var(0),Core::Var(1)]), Core::Tuple(vec![Core::Var(0)]),
        Core::Prim(Prim::ArrGet,vec![Core::Var(0),Core::Var(1)]),
        Core::Proj(Box::new(Core::Var(0)),3), Core::Lam(99,Box::new(Core::Var(0))),
        Core::Match(Box::new(Core::Var(0)),vec![(77,vec![98,99],Core::Var(1)),(88,vec![],Core::Var(2))])];
    for node in nodes {
        let mut order=Vec::new();
        let same=map_children(&node,&mut |child| { order.push(child.clone()); child.clone() });
        assert_eq!(same,node); assert_eq!(order,node.kids().into_iter().cloned().collect::<Vec<_>>());
        let shifted=map_children(&node,&mut |child| child.rename(&mut |v|v+10));
        assert_eq!(shifted.kids().into_iter().cloned().collect::<Vec<_>>(),order.iter().map(|x|x.rename(&mut |v|v+10)).collect::<Vec<_>>());
    }
}
#[test]
fn tail_tokens_are_branch_local_and_never_cross_value_branches_or_calls() {
    let m=CoreModule { ctors:vec![("Pair".into(),2)], ..CoreModule::default() };
    let unbox=HashMap::new(); let cx=ReuseCtx { m:&m,unbox:&unbox,uses:HashMap::from([(0,1)]) };
    let pair=Core::Ctor(0,vec![Core::Num(1),Core::Num(2)]);
    let branch=Core::If(Box::new(Core::Num(1)),Box::new(pair.clone()),Box::new(pair.clone()));
    let mut avail=vec![(42,2)];
    let Core::If(_,yes,no)=reuse(&branch,&cx,&mut avail,true) else { panic!() };
    assert!(matches!(*yes,Core::Reuse(42,0,_))); assert!(matches!(*no,Core::Reuse(42,0,_))); assert!(avail.is_empty());
    let mut avail=vec![(42,2)]; assert_eq!(reuse(&branch,&cx,&mut avail,false),branch); assert!(avail.is_empty());
    let call=Core::Let(9,Box::new(Core::Call(77,vec![])),Box::new(pair.clone()));
    let mut avail=vec![(42,2)]; assert_eq!(reuse(&call,&cx,&mut avail,true),call); assert!(avail.is_empty());
    let lambda=Core::Lam(9,Box::new(pair.clone()));
    let mut avail=vec![(42,2)]; assert_eq!(reuse(&lambda,&cx,&mut avail,true),lambda); assert_eq!(avail,vec![(42,2)]);
    let matched=Core::Match(Box::new(Core::Var(0)),vec![(0,vec![1,2],pair.clone())]);
    let Core::Match(_,arms)=reuse(&matched,&cx,&mut Vec::new(),true) else { panic!() };
    assert!(matches!(arms[0].2,Core::Reuse(0,0,_)));
    assert_eq!(reuse(&matched,&cx,&mut Vec::new(),false),matched);
}
