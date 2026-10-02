use super::*;
#[test]
fn copies_preserve_old_values_and_leave_cycles_and_effects_alone() {
    let live = BTreeSet::from(["arg".into(), "saved".into(), "selector".into()]);
    let body = vec![
        set("alias", v("saved")),
        set("arg", v("alias")),
        set("saved", v("value")),
        set("selector", u32_(1)),
    ];
    let got = canonical(&body, &live).unwrap();
    assert_eq!(got[0], set("arg", v("saved")));
    assert!(got.contains(&set("saved", v("value"))));
    assert!(got.contains(&set("selector", u32_(1))));
    assert!(!got.iter().any(|s| matches!(s, S::Set(n, _) if n=="alias")));
    assert!(canonical(
        &[
            set("temp", v("saved")),
            set("saved", v("arg")),
            set("arg", v("temp"))
        ],
        &live
    )
    .is_none());
    for helper in ["dup_val", "free_val", "arr_set_u", "native_reserve"] {
        assert!(canonical(&[set("arg", c(helper, vec![v("saved")]))], &live).is_none());
    }
    assert!(canonical(&[set("arg", bin(Bop::Div, i64_(7), v("saved")))], &live).is_none());
}
#[test]
fn read_only_paths_do_not_duplicate_computation() {
    let load = c("field", vec![v("node"), usize_(0)]);
    let body = vec![
        set("alias", load.clone()),
        set("arg", v("alias")),
        set("saved", v("alias")),
    ];
    assert!(canonical(&body, &BTreeSet::from(["arg".into(), "saved".into()])).is_none());
    let mut spare = body.clone();
    spare.push(set("dead", bin(Bop::Add, v("x"), v("y"))));
    assert!(canonical(&spare, &BTreeSet::from(["arg".into(), "saved".into()])).is_none());
    assert_eq!(
        canonical(&body, &BTreeSet::from(["arg".into()])),
        Some(vec![set("arg", load)])
    );
}
#[test]
fn selector_updates_share_only_when_all_cases_prove_the_same_delta() {
    let path = |k| Block {
        body: vec![
            set("arg", v("saved")),
            set("saved", v("result")),
            set("selector", u32_(k)),
        ],
        end: End::Jump(5),
    };
    let mut m = Machine {
        funcs: Box::leak(Box::new(BTreeMap::new())),
        blocks: vec![
            Block {
                body: vec![],
                end: End::Switch(v("selector"), vec![(0, 1), (2, 2), (4, 3), (6, 4)], None),
            },
            path(1),
            path(3),
            path(5),
            path(7),
            Block {
                body: vec![],
                end: End::Return(vec![]),
            },
        ],
        locals: BTreeMap::new(),
        params: BTreeMap::new(),
        entries: BTreeMap::new(),
        active: vec![],
        loops: BTreeSet::new(),
        bounds: Box::leak(Box::new(BTreeMap::new())),
        narrow: BTreeSet::new(),
    };
    selectors(&mut m);
    for b in &m.blocks[1..5] {
        assert_eq!(
            b.body[2],
            set(
                "selector",
                cast(bin(Bop::Xor, cast(v("selector"), Ty::I64), i64_(1)), Ty::U32)
            )
        );
    }
    m.blocks[1] = path(1);
    m.blocks[2] = path(3);
    m.blocks[3] = path(9);
    m.blocks[4] = path(7);
    let before = m.blocks[1].body.clone();
    selectors(&mut m);
    assert_eq!(m.blocks[1].body, before);
    m.blocks[3] = path(5);
    m.blocks.push(Block {
        body: vec![],
        end: End::Jump(1),
    });
    let before = m.blocks[1].body.clone();
    selectors(&mut m);
    assert_eq!(
        m.blocks[1].body, before,
        "a shared incoming path has no case-key proof"
    );
}
#[test]
fn sharing_keeps_default_and_exceptional_paths_and_selector_types() {
    let path = |key| {
        vec![
            set("arg", c("field", vec![v("node"), usize_(0)])),
            set("selector", u32_(key)),
            S::Jump(u64_(9)),
        ]
    };
    let exception = vec![ret(i64_(-17))];
    let mut body = vec![S::Switch(
        v("op"),
        vec![(0, exception.clone()), (2, path(0)), (3, path(2))],
        Some(path(6)),
    )];
    factor(&mut body);
    let S::Switch(_, arms, Some(default)) = &body[0] else {
        panic!("switch lost")
    };
    assert_eq!(arms, &vec![(0, exception)]);
    assert_eq!(
        default[0],
        S::Switch(
            v("op"),
            vec![
                (2, vec![set("selector", u32_(0))]),
                (3, vec![set("selector", u32_(2))])
            ],
            Some(vec![set("selector", u32_(6))])
        )
    );
    assert_eq!(&default[1..], &[path(6)[0].clone(), S::Jump(u64_(9))]);
    // A selector read or an effect before its assignment prevents motion.
    for prefix in [
        set("arg", v("selector")),
        set("selector", u32_(17)),
        do_(c("free_val", vec![v("node")])),
    ] {
        let p = |key| vec![prefix.clone(), set("selector", u32_(key)), S::Jump(u64_(9))];
        let mut b = vec![S::Switch(v("op"), vec![(2, p(0))], Some(p(6)))];
        let original = b.clone();
        factor(&mut b);
        assert_eq!(b, original);
    }
}
