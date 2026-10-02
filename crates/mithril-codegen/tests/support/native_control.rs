use super::*;

#[test]
fn control_splicing_preserves_branches_order_and_scope_boundaries() {
    let private = FnDef { name: "private".into(), ctx: false, params: vec![], ret: Ty::I64,
        body: jump(7), inline: Inline::Default, cold: false };
    let nested = vec![S::Loop(jump(7)), S::Fn(Box::new(private))];
    let source = vec![set("before", i64_(1)),
        S::If(v("condition"), jump(7), vec![S::Switch(v("tag"), vec![(3, jump(8))], Some(jump(7)))]),
        nested[0].clone(), nested[1].clone()];
    let mapped = map_control(&source, &mut |s| Some(match s {
        S::Jump(E::Int(7, _)) => vec![set("first", i64_(7)), set("second", i64_(8))],
        S::Jump(E::Int(8, _)) => vec![],
        s => vec![s.clone()],
    })).unwrap();
    let replacement = vec![set("first", i64_(7)), set("second", i64_(8))];
    assert_eq!(mapped, vec![set("before", i64_(1)),
        S::If(v("condition"), replacement.clone(),
            vec![S::Switch(v("tag"), vec![(3, vec![])], Some(replacement))]),
        nested[0].clone(), nested[1].clone()]);
}

#[test]
fn failed_control_projection_leaves_the_original_paths_intact() {
    let source = vec![S::If(v("condition"), jump(7), vec![S::Jump(v("dynamic"))])];
    let before = source.clone();
    assert!(map_control(&source, &mut |s| match s {
        S::Jump(E::Int(..)) => Some(vec![S::Continue]),
        S::Jump(_) => None,
        s => Some(vec![s.clone()]),
    }).is_none());
    assert_eq!(source, before);
}

#[test]
fn structured_cycles_require_one_loop_and_acyclic_static_entry_paths() {
    let arms = vec![(0, vec![set("entry", i64_(1)), S::Jump(u64_(1))]),
        (1, vec![S::Loop(vec![S::If(v("done"), vec![ret(v("entry"))], jump(2))])]),
        (2, vec![set("step", i64_(2)), S::Jump(u64_(1))])];
    assert_eq!(structured_cycle(&arms, 0), Some(vec![set("entry", i64_(1)),
        S::Loop(vec![S::If(v("done"), vec![ret(v("entry"))], vec![set("step", i64_(2)), S::Continue])])]));
    let mut cyclic = arms.clone();
    cyclic[2].1[1] = S::Jump(u64_(2));
    assert!(structured_cycle(&cyclic, 0).is_none());
    let mut dynamic = arms.clone();
    dynamic[1].1 = vec![S::Loop(vec![S::Jump(v("dynamic"))])];
    assert!(structured_cycle(&dynamic, 0).is_none());
    let mut two = arms;
    two[2].1 = vec![S::Loop(vec![S::Continue])];
    assert!(structured_cycle(&two, 0).is_none());
}
