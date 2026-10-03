use super::{derive_set, escape_mask};
use mithril_front::core::{Core, Prim};
use std::collections::HashSet;

fn v(x: u32) -> Core { Core::Var(x) }
fn bind(x: u32, rhs: Core, body: Core) -> Core {
    Core::Let(x, Box::new(rhs), Box::new(body))
}
fn matched(source: Core, fields: Vec<u32>, body: Core) -> Core {
    Core::Match(Box::new(source), vec![(0, fields, body)])
}
fn escape(body: &Core, own: &[bool], callees: &[Vec<bool>], ints: &[u32]) -> u64 {
    escape_mask(u32::MAX, body, own.len(), own, callees, &|x| ints.contains(&x))
}

#[test]
fn provenance_follows_aliases_and_boxed_fields_but_stops_at_integer_fields() {
    let body = bind(2, v(0), matched(v(2), vec![3, 4], Core::Tuple(vec![v(3), v(4), v(1)])));
    assert_eq!(escape(&body, &[true, true], &[], &[3]), 3);
    assert_eq!(escape(&body, &[true, true], &[], &[3, 4]), 2);
    assert_eq!(derive_set(&body, &[true, true]), HashSet::from([0, 1, 2, 3, 4]));
    let returned = bind(2, v(0), v(2));
    assert_eq!(escape(&returned, &[true], &[], &[]), 0);
    assert_eq!(derive_set(&returned, &[true]), HashSet::from([0, 2]));
}

#[test]
fn storage_and_parameter_ownership_define_escape_boundaries() {
    let cases = [
        (Core::Ctor(0, vec![v(0), v(1)]), 3),
        (Core::Reuse(9, 0, vec![v(0), v(1)]), 3),
        (Core::Tuple(vec![v(0), v(1)]), 3),
        (Core::Prim(Prim::ArrNew, vec![v(0), v(1)]), 3),
        (Core::Prim(Prim::ArrSet, vec![v(0), v(1), v(2)]), 7),
        (Core::Prim(Prim::ArrGet, vec![v(0), v(1)]), 2),
        (Core::Prim(Prim::ArrLen, vec![v(0)]), 0),
        (Core::Call(0, vec![v(0), v(1), v(2)]), 6),
        (Core::Call(8, vec![v(0), v(1)]), 3),
    ];
    for (body, expected) in cases {
        assert_eq!(escape(&body, &[true; 3], &[vec![true, false]], &[]), expected, "{body:?}");
    }
    assert_eq!(escape_mask(u32::MAX, &v(0), 61, &[false; 61], &[], &|_| false), u64::MAX);
}

#[test]
fn a_self_call_carrying_an_owned_value_in_a_lent_slot_unlends_it() {
    // f(a, k): a self call passing a fresh array where `a` was lent would make
    // the slot hold an owned value no one frees; passing `a` itself, or a field
    // of a lent value, keeps the slot lent.
    let lent = [vec![true, false]];
    let fresh = bind(2, Core::Prim(Prim::ArrNew, vec![v(1), v(1)]), Core::Call(0, vec![v(2), v(1)]));
    assert_eq!(escape_mask(0, &fresh, 2, &[true, false], &lent, &|_| false), 1);
    let same = Core::Call(0, vec![v(0), v(1)]);
    assert_eq!(escape_mask(0, &same, 2, &[true, false], &lent, &|_| false), 0);
    let field = matched(v(0), vec![2], Core::Call(0, vec![v(2), v(1)]));
    assert_eq!(escape_mask(0, &field, 2, &[true, false], &lent, &|_| false), 0);
    // another function's call is judged by its own modes, not by this rule
    assert_eq!(escape_mask(1, &fresh, 2, &[true, false], &lent, &|_| false), 0);
}

#[test]
fn raw_read_membership_is_a_union_when_branch_binders_are_reused() {
    let fields = |source| matched(v(source), vec![2], Core::Tuple(vec![v(2)]));
    let body = Core::If(Box::new(Core::Num(1)), Box::new(fields(0)), Box::new(fields(1)));
    assert_eq!(derive_set(&body, &[true, false]), HashSet::from([0, 2]));
    assert_eq!(escape(&body, &[true, false], &[], &[]), 1);
    let body = Core::If(Box::new(Core::Num(1)), Box::new(bind(2, v(0), v(2))), Box::new(bind(2, v(1), v(2))));
    assert_eq!(derive_set(&body, &[true, false]), HashSet::from([0, 2]));
}
