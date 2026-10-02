use super::*;

#[test]
fn frame_layout_pairs_every_offset_width_and_restore_type() {
    let funcs = BTreeMap::new();
    let bounds = BTreeMap::new();
    let mut m = Machine::new(&funcs, &bounds, None);
    m.locals = BTreeMap::from([("a".into(), Ty::I64), ("b".into(), Ty::U64), ("c".into(), Ty::Bool)]);
    m.narrow = BTreeSet::from(["a".into(), "c".into()]);
    let names = ["a".to_string(), "b".to_string(), "c".to_string()];
    let frame = Frame::new(&m, &names);
    assert_eq!(frame.width, 4);
    for restore in [false, true] {
        let slot = if restore { "restore_slot" } else { "save_slot" };
        let S::If(condition, fixed, dynamic) = frame.transfer(restore) else { panic!("cache dispatch missing") };
        assert_eq!(condition, p("native_cached", vec![E::Addr("frames".into()), v(slot)]));
        for (suffix, body) in [("_fixed", fixed), ("", dynamic)] {
            let args = |offset| vec![E::Ref("frames".into()), v(slot), u32_(offset)];
            let expected = if restore {
                vec![set("c", cast(p(&format!("native_get{suffix}32"), args(3)), Ty::Bool)),
                    set("b", cast(p(&format!("native_get{suffix}64"), args(1)), Ty::U64)),
                    set("a", cast(p(&format!("native_get{suffix}32"), args(0)), Ty::I64))]
            } else {
                vec![do_(p(&format!("native_set{suffix}32"), [args(0), vec![cast(v("a"), Ty::U32)]].concat())),
                    do_(p(&format!("native_set{suffix}64"), [args(1), vec![cast(v("b"), Ty::U64)]].concat())),
                    do_(p(&format!("native_set{suffix}32"), [args(3), vec![cast(v("c"), Ty::U32)]].concat()))]
            };
            assert_eq!(body, expected);
        }
    }
    let empty = Frame::new(&m, &[]);
    assert_eq!(empty.width, 1);
    for restore in [false, true] {
        let S::If(_, fixed, dynamic) = empty.transfer(restore) else { unreachable!() };
        for (suffix, body) in [("_fixed", fixed), ("", dynamic)] {
            let mut args = vec![E::Ref("frames".into()), v(if restore { "restore_slot" } else { "save_slot" }), u32_(0)];
            if !restore { args.push(u32_(0)); }
            assert_eq!(body, vec![do_(p(&format!("native_{}{suffix}32", if restore { "get" } else { "set" }), args))]);
        }
    }
}

#[test]
fn storage_rewrites_reads_before_writes_and_respects_suspension_scopes() {
    let private = FnDef { name: "private".into(), ctx: false, params: vec![], ret: Ty::I64,
        body: vec![ret(v("word"))], inline: Inline::Default, cold: false };
    let try_ = S::Try(Pat::One("out".into()), v("word"), "err".into(), vec![ret(v("word"))]);
    let res = S::Res(v("word"), "ok".into(), vec![ret(v("word"))], "err".into(), vec![ret(v("word"))]);
    let mut body = vec![S::Decl("word".into(), Ty::I64), set("word", i64_(1)),
        S::If(v("word"), vec![set("word", bin(Bop::Add, v("word"), i64_(1)))], vec![]),
        S::Switch(v("word"), vec![(0, vec![ret(v("word"))])], Some(vec![])),
        S::Loop(vec![S::Machine(vec![(0, vec![S::Jump(v("word"))])])]),
        try_.clone(), res.clone(), S::Fn(Box::new(private.clone()))];
    apply(&mut body, &BTreeSet::from(["word".into()]));
    assert_eq!(body[0], S::Decl("word".into(), Ty::U32));
    assert_eq!(body[1], set("word", cast(i64_(1), Ty::U32)));
    assert_eq!(body[2], S::If(cast(v("word"), Ty::I64),
        vec![set("word", cast(bin(Bop::Add, cast(v("word"), Ty::I64), i64_(1)), Ty::U32))], vec![]));
    assert_eq!(body[3], S::Switch(cast(v("word"), Ty::I64),
        vec![(0,vec![ret(cast(v("word"), Ty::I64))])], Some(vec![])));
    assert_eq!(body[4], S::Loop(vec![S::Machine(vec![(0,vec![S::Jump(cast(v("word"), Ty::I64))])]) ]));
    assert_eq!(body[5], try_); assert_eq!(body[6], res); assert_eq!(body[7], S::Fn(Box::new(private)));
}
