use mithril_codegen::lir::*;

#[test]
fn adjacent_borrowed_reads_share_one_cell_and_preserve_conversions() {
    let read = |node, i| c("field", vec![v(node), usize_(i)]);
    let mut body = vec![S::If(
        E::Bool(true),
        vec![
            set("a", cast(read("node", 0), Ty::I64)),
            set("b", as_i(read("node", 1))),
        ],
        vec![],
    )];
    borrowed_cells(&mut body);
    let S::If(_, branch, _) = &body[0] else {
        panic!("branch lost")
    };
    let S::Let(Pat::Arr(names), Ty::Infer, call) = &branch[0] else {
        panic!("shared read missing")
    };
    assert_eq!(call, &c("read_pair", vec![v("node")]));
    assert_eq!(branch[1], set("a", cast(v(&names[0]), Ty::I64)));
    assert_eq!(branch[2], set("b", as_i(v(&names[1]))));
}

#[test]
fn writes_effects_different_cells_and_repeated_fields_are_barriers() {
    let read = |node, i| c("field", vec![v(node), usize_(i)]);
    for mut body in [
        vec![set("node", read("node", 0)), set("b", read("node", 1))],
        vec![set("a", read("node", 0)), set("b", read("other", 1))],
        vec![set("a", read("node", 0)), set("b", read("node", 0))],
        vec![
            set("a", read("node", 0)),
            do_(c("free_val", vec![v("node")])),
            set("b", read("node", 1)),
        ],
        vec![
            set("a", c("free_val", vec![read("node", 0)])),
            set("b", read("node", 1)),
        ],
    ] {
        let original = body.clone();
        borrowed_cells(&mut body);
        assert_eq!(body, original);
    }
}

#[test]
fn pair_temporaries_do_not_shadow_existing_locals_or_repeat_effectful_sources() {
    let mut body = vec![
        let_("borrow_0_0", Ty::U64, u64_(19)),
        set("a", c("field", vec![v("node"), usize_(0)])),
        set("b", c("field", vec![v("node"), usize_(1)])),
        ret(v("borrow_0_0")),
    ];
    borrowed_cells(&mut body);
    let S::Let(Pat::Arr(names), _, _) = &body[1] else {
        panic!("pair missing")
    };
    assert!(!names.contains(&"borrow_0_0".into()));
    let source = c("dup_val", vec![v("node")]);
    let mut body = vec![
        set("a", c("field", vec![source.clone(), usize_(0)])),
        set("b", c("field", vec![source, usize_(1)])),
    ];
    let original = body.clone();
    borrowed_cells(&mut body);
    assert_eq!(body, original);
}
