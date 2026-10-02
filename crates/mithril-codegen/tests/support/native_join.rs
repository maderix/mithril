use super::*;
fn machine() -> Machine<'static> {
    Machine {
        funcs: Box::leak(Box::new(BTreeMap::new())),
        blocks: vec![
            Block {
                body: vec![],
                end: End::Branch(v("flag"), 1, 2),
            },
            Block {
                body: vec![set("a", bin(Bop::Add, v("flag"), i64_(7)))],
                end: End::Call("s_0".into(), vec![v("left")], vec!["x".into()], 3),
            },
            Block {
                body: vec![set("b", bin(Bop::Sub, v("flag"), i64_(11)))],
                end: End::Call("s_0".into(), vec![v("right")], vec!["y".into()], 4),
            },
            Block {
                body: vec![],
                end: End::Return(vec![bin(Bop::Sub, v("a"), v("x"))]),
            },
            Block {
                body: vec![],
                end: End::Return(vec![bin(Bop::Mul, v("b"), v("y"))]),
            },
        ],
        locals: ["flag", "a", "b", "x", "y", "left", "right", "param"]
            .into_iter()
            .map(|n| (n.into(), Ty::I64))
            .collect(),
        params: BTreeMap::from([("s_0".into(), vec!["param".into()])]),
        entries: BTreeMap::from([("s_0".into(), 0)]),
        active: vec![],
        loops: BTreeSet::new(),
        bounds: Box::leak(Box::new(BTreeMap::new())),
        narrow: BTreeSet::new(),
    }
}
#[test]
fn equal_capture_layouts_share_one_call_and_keep_distinct_return_paths() {
    let mut m = machine();
    share(&mut m);
    let calls: Vec<_> = m
        .blocks
        .iter()
        .filter_map(|b| {
            if let End::Call(f, args, outs, next) = &b.end {
                Some((f, args, outs, *next))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(calls.len(), 1);
    let (f, args, outs, next) = calls[0];
    assert_eq!(f, "s_0");
    assert_eq!(args.len(), 1);
    assert_eq!(outs.len(), 1);
    let End::Switch(selector, arms, None) = &m.blocks[next].end else {
        panic!("return paths missing")
    };
    let E::V(selector) = selector else {
        panic!("selector missing")
    };
    assert!(m.narrow.contains(selector));
    assert_eq!(arms.len(), 2);
    for (site, (_, restore)) in arms.iter().enumerate() {
        assert!(matches!(m.blocks[site + 1].end, End::Jump(_)));
        assert!(matches!(m.blocks[*restore].end,End::Jump(n) if n==site+3));
        assert!(m.blocks[*restore]
            .body
            .iter()
            .any(|s| matches!(s,S::Set(n,_) if n==if site==0 {"a"} else {"b"})));
        assert!(m.blocks[*restore]
            .body
            .iter()
            .any(|s| matches!(s,S::Set(n,_) if n==if site==0 {"x"} else {"y"})));
    }
}
#[test]
fn different_callees_widths_and_capture_bounds_keep_separate_calls() {
    for different in 0..4 {
        let mut m = machine();
        match different {
            0 => {
                if let End::Call(f, _, _, _) = &mut m.blocks[2].end {
                    *f = "s_1".into();
                }
                m.params.insert("s_1".into(), vec!["param".into()]);
            }
            1 => {
                m.locals.insert("b".into(), Ty::U64);
            }
            2 => {
                m.narrow.insert("a".into());
            }
            _ => {
                m.blocks[4].end = End::Return(vec![v("y")]);
            }
        }
        share(&mut m);
        assert_eq!(
            m.blocks.iter().filter(|b| matches!(b.end, End::Call(..))).count(),
            2
        );
    }
}
