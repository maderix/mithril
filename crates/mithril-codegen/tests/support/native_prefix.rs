use super::*;
use mithril_front::core::{Core, CoreFn};
fn forms() -> (FnDef, FnDef, CoreModule, Vec<Option<Sig>>) {
    let f = FnDef {
        name: "s_0".into(),
        ctx: false,
        params: vec![("fuel".into(), Ty::RefI64), ("v0".into(), Ty::I64)],
        ret: Ty::I64,
        body: vec![
            let_("fl", Ty::I64, i64_(1)),
            let_("r", Ty::I64, p("s_1", vec![v("fuel"), v("v0")])),
            do_(p("work_fuel", vec![v("fuel"), v("fl")])),
            ret(v("r")),
        ],
        inline: Inline::Default,
        cold: false,
    };
    let mut d = f.clone();
    d.name = "region_d_0".into();
    d.ctx = true;
    d.params[1].1 = Ty::U64;
    d.ret = Ty::Res;
    d.body = vec![
        do_(p("stack_guard", vec![])),
        burn_fuel(),
        S::If(
            bin(Bop::Lt, E::Deref("fuel".into()), i64_(0)),
            vec![ret(E::Err(Box::new(u64_(0))))],
            vec![],
        ),
        ret(c("d_1", vec![v("fuel"), v("v0")])),
    ];
    let m = CoreModule {
        fns: (0..2)
            .map(|i| CoreFn {
                name: format!("f{i}"),
                arity: 1,
                body: Core::Num(0),
                self_tail_rec: false,
                fold: None,
            })
            .collect(),
        ..CoreModule::default()
    };
    let sigs = vec![
        Some(Sig {
            params: vec![PTy::I],
            ret: Kind::S1,
            ra: vec![false]
        });
        2
    ];
    (f, d, m, sigs)
}
#[test]
fn prefix_constructs_a_closed_route_domain_and_preserves_charge_protocols() {
    let (mut f, mut d, m, sigs) = forms();
    let burn = d.body[..3].to_vec();
    let prefix = share(&mut f, &mut d, &m, &sigs).expect("integer tail is shareable");
    assert_eq!(&d.body[..3], &burn);
    let mut tags = BTreeSet::new();
    walk_stmts(&prefix.body, &mut |s| {
        if let S::Ret(E::Tup(xs)) = s {
            let E::Int(tag, Ty::I64) = xs[0] else {
                panic!("route must be constructed as a literal")
            };
            tags.insert(tag);
        }
    });
    assert_eq!(tags, BTreeSet::from([2]));
    for body in [&f.body, &d.body] {
        let S::Switch(_, arms, Some(default)) = body.last().unwrap() else {
            panic!("closed dispatch needs an ordinary returning default")
        };
        assert_eq!(arms.iter().map(|(tag, _)| *tag).collect::<Vec<_>>(), vec![0]);
        assert!(matches!(default.last(), Some(S::Ret(_))));
        assert!(!default.iter().any(|s| matches!(s, S::Unreachable)));
    }
    assert!(
        matches!(f.body.last(),Some(S::Switch(_,_,Some(b))) if b.iter().any(charge) || b.iter().any(|s|matches!(s,S::Do(E::Call{f,..}) if f=="work_fuel")))
    );
}
#[test]
fn unsupported_or_observing_lir_declines_without_mutating_either_entry() {
    for kind in 0..12 {
        let (mut f, mut d, m, mut sigs) = forms();
        match kind {
            0 => f.body.insert(1, do_(p("unknown", vec![]))),
            1 => f.body.insert(1, let_("x", Ty::I64, E::Deref("fuel".into()))),
            2 => f.body[3] = ret(bin(Bop::Add, v("r"), i64_(1))),
            3 => f.body[1] = let_("r", Ty::I64, c("s_1", vec![v("fuel"), v("v0")])),
            4 => f.body[1] = let_("r", Ty::I64, p("s_1", vec![v("fuel"), v("fuel")])),
            5 => f.body[2] = do_(p("work_fuel", vec![v("fuel"), i64_(2)])),
            6 => d.body.insert(3, do_(p("unknown_effect", vec![]))),
            7 => d
                .body
                .insert(3, let_("observed", Ty::I64, E::Deref("fuel".into()))),
            8 => d.body[3] = ret(p("s_1", vec![v("fuel"), v("v0")])),
            9 => sigs[1] = None,
            10 => {
                f.ret = Ty::Tup(0);
                d.ret = Ty::Res;
            }
            11 => d.ret = Ty::ResArr(1),
            _ => unreachable!(),
        }
        let before = (f.clone(), d.clone());
        assert!(share(&mut f, &mut d, &m, &sigs).is_none(), "case {kind}");
        assert_eq!((f, d), before, "rejection must be transactional");
    }
}

#[test]
fn branch_domain_includes_literals_and_each_original_callee_protocol() {
    let (mut f, mut d, mut m, mut sigs) = forms();
    m.fns.push(m.fns[1].clone());
    sigs.push(sigs[1].clone());
    let native_tail = |g| {
        vec![
            let_("r", Ty::I64, p(&format!("s_{g}"), vec![v("fuel"), v("v0")])),
            do_(p("work_fuel", vec![v("fuel"), v("fl")])),
            ret(v("r")),
        ]
    };
    let condition = bin(Bop::Eq, v("v0"), i64_(0));
    f.body = vec![
        let_("fl", Ty::I64, i64_(1)),
        S::If(
            condition.clone(),
            vec![do_(p("work_fuel", vec![v("fuel"), v("fl")])), ret(i64_(9))],
            vec![S::If(
                bin(Bop::Lt, v("v0"), i64_(3)),
                native_tail(1),
                native_tail(2),
            )],
        ),
    ];
    d.body.truncate(3);
    d.body.push(S::If(
        condition,
        vec![ret(ok(num(i64_(9))))],
        vec![S::If(
            bin(Bop::Lt, v("v0"), i64_(3)),
            vec![ret(c("q_1", vec![v("fuel"), v("v0")]))],
            vec![ret(c("d_2", vec![v("fuel"), v("v0")]))],
        )],
    ));
    let prefix = share(&mut f, &mut d, &m, &sigs).unwrap();
    let mut tags = BTreeSet::new();
    walk_stmts(&prefix.body, &mut |s| {
        if let S::Ret(E::Tup(xs)) = s {
            let E::Int(tag, Ty::I64) = xs[0] else {
                panic!("constructed route must be literal")
            };
            tags.insert(tag);
        }
    });
    assert_eq!(tags, BTreeSet::from([0, 2, 3]));
    let mut protocols = BTreeSet::new();
    walk_stmts(&d.body[3..], &mut |s| {
        if let Some(e) = s.parts().0 {
            e.walk(&mut |e| {
                if let E::Call { f, ctx: true, .. } = e {
                    protocols.insert(f.clone());
                }
            });
        }
    });
    assert_eq!(protocols, BTreeSet::from(["q_1".into(), "d_2".into()]));
    for body in [&f.body, &d.body] {
        let S::Switch(_, arms, Some(default)) = body.last().unwrap() else {
            panic!("certified returning dispatch")
        };
        assert_eq!(arms.iter().map(|(tag, _)| *tag).collect::<Vec<_>>(), vec![0, 2]);
        assert!(matches!(default.last(), Some(S::Ret(_))));
    }
}
