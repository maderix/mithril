use super::*;

fn machine(blocks: Vec<Block>) -> Machine<'static> {
    let mut m = Machine::new(Box::leak(Box::new(BTreeMap::new())), Box::leak(Box::new(BTreeMap::new())), None);
    m.blocks = blocks;
    m.locals = ["flag", "arg", "out", "carry", "wide", "constant", "p", "r", "d"]
        .into_iter().map(|n| (n.into(), Ty::I64)).collect();
    m.params.insert("s_0".into(), vec!["p".into()]);
    m
}
fn block(body: Vec<S>, end: End) -> Block { Block { body, end } }
fn names(xs: &[&str]) -> BTreeSet<String> { xs.iter().map(|n| n.to_string()).collect() }

#[test]
fn graph_edges_preserve_default_duplicate_and_call_resume_targets() {
    let m = machine(vec![
        block(vec![], End::Branch(v("flag"), 1, 1)),
        block(vec![], End::Switch(v("arg"), vec![(0, 2), (3, 2)], Some(3))),
        block(vec![], End::Call("s_0".into(), vec![v("arg")], vec!["out".into()], 3)),
        block(vec![], End::Return(vec![v("out")])),
    ]);
    assert_eq!(m.incoming(), vec![0, 2, 2, 2]);
    assert_eq!(m.blocks[0].end.targets(), vec![1, 1]);
    assert_eq!(m.blocks[1].end.targets(), vec![2, 2, 3]);
    assert_eq!(m.blocks[2].end.targets(), vec![3]);
    assert!(m.blocks[3].end.targets().is_empty());
    assert_eq!(m.blocks[2].end.exprs(), &[v("arg")]);
    assert_eq!(m.blocks[3].end.exprs(), &[v("out")]);
}

#[test]
fn liveness_finds_cycle_join_pointer_and_assignment_snapshot_inputs() {
    let m = machine(vec![
        block(vec![set("carry", v("wide"))], End::Jump(1)),
        block(vec![set("carry", bin(Bop::Add, v("carry"), i64_(1)))], End::Branch(v("flag"), 1, 2)),
        block(vec![S::Let(Pat::One("out".into()), Ty::I64, E::Deref("d".into()))],
            End::Return(vec![E::Tup(vec![v("carry"), v("out"), E::Ref("r".into()), E::Addr("p".into())])])),
    ]);
    let live = m.live();
    assert_eq!(live[0], names(&["wide", "flag", "d", "r", "p"]));
    assert_eq!(live[1], names(&["carry", "flag", "d", "r", "p"]));
    assert_eq!(live[2], names(&["carry", "d", "r", "p"]));
}

#[test]
fn captures_exclude_results_invariants_and_dead_values_but_keep_live_widths() {
    let mut m = machine(vec![
        block(vec![set("constant", i64_(7)), set("carry", bin(Bop::Add, v("arg"), i64_(1)))],
            End::Call("s_0".into(), vec![v("p")], vec!["out".into()], 1)),
        block(vec![], End::Return(vec![E::Tup(vec![v("carry"), v("out"), v("p"), v("constant"), v("wide")])])),
    ]);
    m.locals.insert("carry".into(), Ty::U64);
    m.narrow.insert("wide".into());
    let captures = m.captures();
    assert_eq!(captures[&0], vec!["carry".to_string(), "wide".to_string()]);
    assert_eq!(storage::Frame::layout(&m, &captures[&0]), vec![(Ty::U64, 2), (Ty::I64, 1)]);
    assert!(!captures.contains_key(&1));
}

#[test]
fn facts_recompute_after_graph_body_and_width_changes() {
    let mut m = machine(vec![
        block(vec![set("carry", bin(Bop::Add, v("arg"), i64_(1)))],
            End::Call("s_0".into(), vec![v("p")], vec!["out".into()], 1)),
        block(vec![], End::Return(vec![v("carry")])),
        block(vec![], End::Return(vec![v("wide")])),
    ]);
    assert_eq!(m.captures()[&0], vec!["carry".to_string()]);
    m.blocks[0].end = End::Call("s_0".into(), vec![v("p")], vec!["out".into()], 2);
    assert_eq!(m.incoming(), vec![0, 0, 1]);
    assert_eq!(m.captures()[&0], vec!["wide".to_string()]);
    m.blocks[2].body.push(set("wide", v("out")));
    assert!(m.captures()[&0].is_empty());
    m.blocks[2].body.clear();
    let saved = m.captures()[&0].clone();
    assert_eq!(storage::Frame::layout(&m, &saved), vec![(Ty::I64, 2)]);
    m.narrow.insert("wide".into());
    assert_eq!(storage::Frame::layout(&m, &saved), vec![(Ty::I64, 1)]);
}
