use mithril_codegen::lir::{operations::{helper, Effect, Policy}, *};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn unknown_and_effectful_helpers_cannot_enter_expression_shortcuts() {
    for call in [p("unknown", vec![]), c("cell_set", vec![]), c("free_val", vec![]),
        c("dup_val", vec![]), c("alloc2", vec![]), p("work_fuel", vec![]),
        p("native_reserve", vec![])] {
        for policy in [Policy::Copy, Policy::Exit, Policy::Prefix] {
            assert!(!policy.accepts(&call), "{policy:?} allowed {call:?}");
        }
    }
    assert!(helper("unknown").is_none());
    assert_eq!(helper("field").unwrap().effect, Effect::Borrow);
    assert_eq!(helper("read_pair").unwrap().result, Ty::Arr(2));
    assert_eq!(helper("cell_set").unwrap().effect, Effect::Write);
    assert_eq!(helper("free_val").unwrap().effect, Effect::Own);
    assert_eq!(helper("alloc2").unwrap().effect, Effect::Allocate);
    assert_eq!(helper("work_fuel").unwrap().effect, Effect::Fuel);
}

#[test]
fn proof_policies_distinguish_borrowed_paths_total_exits_and_prefixes() {
    let cases = [
        (p("wrap56", vec![v("x")]), [true, true, true]),
        (c("wrap56", vec![v("x")]), [false, false, false]),
        (c("field", vec![v("t"), usize_(0)]), [true, false, false]),
        (p("field", vec![v("t"), usize_(0)]), [false, false, false]),
        (p("tag", vec![v("t")]), [true, true, false]),
        (p("as_i", vec![v("x")]), [true, true, true]),
        (p("num", vec![v("x")]), [false, true, true]),
        (p("con_tag", vec![v("t")]), [true, true, false]),
        // releases a boxed int: an ownership effect, admitted by no policy
        (p("take_i", vec![v("x")]), [false, false, false]),
        (p("floor_div", vec![v("x"), v("y")]), [false, false, true]),
        (bin(Bop::Div, v("x"), v("y")), [false, false, true]),
        (E::Tup(vec![v("x")]), [false, false, true]),
        (v("fuel"), [true, true, false]),
        (E::Ref("x".into()), [false, false, false]),
    ];
    for (e, want) in cases {
        for (policy, want) in [Policy::Copy, Policy::Exit, Policy::Prefix].into_iter().zip(want) {
            assert_eq!(policy.accepts(&e), want, "{policy:?}: {e:?}");
        }
    }
    assert!(helper("floor_div").unwrap().may_fail);
    assert!(helper("field").unwrap().may_fail);
    assert!(!helper("wrap56").unwrap().may_fail);
    assert!(!Policy::Copy.accepts(&p("wrap56", vec![c("free_val", vec![v("x")])])));
}

#[test]
fn substitution_uses_assignment_snapshots_and_never_revisits_replacements() {
    let mut env = BTreeMap::from([("x".into(), v("y")), ("y".into(), i64_(7))]);
    let e = bin(Bop::Add, v("x"), v("y"));
    assert_eq!(e.substitute(&env, true), Some(bin(Bop::Add, v("y"), i64_(7))));
    env.insert("y".into(), v("x"));
    assert_eq!(e.substitute(&env, true), Some(bin(Bop::Add, v("y"), v("x"))));
    assert_eq!(v("missing").substitute(&env, true), None);
    assert_eq!(v("missing").substitute(&env, false), Some(v("missing")));
    assert_eq!(E::Ref("x".into()).substitute(&env, false), Some(E::Ref("x".into())));
    let snapshot = e.substitute(&env, false).unwrap();
    env.insert("x".into(), i64_(99));
    assert_eq!(snapshot, bin(Bop::Add, v("y"), v("x")));
    let uses = E::Tup(vec![v("v"), E::Ref("r".into()), E::Addr("a".into()), E::Deref("d".into())]);
    assert_eq!(uses.reads(), BTreeSet::from(["a".into(), "d".into(), "r".into(), "v".into()]));
}
