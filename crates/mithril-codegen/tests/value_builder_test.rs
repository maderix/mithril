use mithril_codegen::lir::{self, as_i, c, free, i64_, let_, num, p, u16_, usize_, v, Pat, Ty, S};

#[path = "../src/value.rs"]
mod value;

#[test]
fn value_bindings_and_integer_representations_preserve_exact_lir() {
    for ty in [Ty::I64, Ty::U64, Ty::Infer] {
        let mut out = vec![S::Comment("prefix".into())];
        let expr = p("side_effect", vec![i64_(-1)]);
        assert_eq!(value::bind("chosen_name", ty, expr.clone(), &mut out), v("chosen_name"));
        assert_eq!(out, vec![S::Comment("prefix".into()), let_("chosen_name", ty, expr)]);
    }
    for e in [i64_(i64::MIN), i64_(i64::MAX), i64_(0), v("r"), c("caller", vec![v("arg")])] {
        assert_eq!(value::int_port(e.clone(), false), num(e.clone()));
        assert_eq!(value::int_port(e.clone(), true), p("retag", vec![e.clone()]));
        assert_eq!(value::native_int(e.clone(), false), as_i(e.clone()));
        assert_eq!(value::native_int(e.clone(), true), p("sh", vec![e]));
    }
}

#[test]
fn constructor_fields_preserve_borrow_consumption_and_reuse_protocols() {
    for n in [0, 1, 2, 3, 9, 47] {
        for cid in [0, 31, 4095] {
            let names: Vec<String> = (0..n).map(|i| format!("f{i}")).collect();
            let mut out = vec![S::Comment("prefix".into())];
            let raw = value::fields(v("src"), cid, names.clone(), false, None, &mut out);
            assert_eq!(raw, (0..n).map(|i| c("field", vec![v("src"), usize_(i)])).collect::<Vec<_>>());
            assert_eq!(out, vec![S::Comment("prefix".into())]);
            out.clear();
            let moved = value::fields(v("src"), cid, names.clone(), true, None, &mut out);
            assert_eq!(moved, names.iter().map(v).collect::<Vec<_>>());
            let (pat, helper) = match n {
                1 => (Pat::Tup(vec!["f0".into(), "m_unused".into()]), "consume2k".into()),
                2 => (Pat::Tup(names), "consume2k".into()),
                n => (Pat::Arr(names), format!("consume_chain::<{n}>")),
            };
            let mut expected = vec![S::Let(pat, Ty::Infer, c(&helper, vec![v("src"), u16_(cid as u64)]))];
            if n == 1 {
                expected.push(free(v("m_unused")));
            }
            assert_eq!(out, expected, "arity={n} cid={cid}");
        }
    }
    let mut out = Vec::new();
    assert_eq!(value::fields(v("src"), 7, vec!["a".into(), "b".into()], true, Some("token".into()), &mut out), vec![v("a"), v("b")]);
    assert_eq!(out, vec![S::Let(Pat::Tup(vec!["a".into(), "b".into(), "token".into()]), Ty::Infer, c("consume2r", vec![v("src"), u16_(7)]))]);
}

#[test]
fn reuse_tokens_require_an_owned_two_field_constructor() {
    for (owned, n) in [(false, 2), (true, 0), (true, 1), (true, 3)] {
        let result = std::panic::catch_unwind(|| {
            let names = (0..n).map(|i| format!("f{i}")).collect();
            value::fields(v("src"), 0, names, owned, Some("token".into()), &mut Vec::new())
        });
        assert!(result.is_err(), "owned={owned} arity={n}");
    }
}
