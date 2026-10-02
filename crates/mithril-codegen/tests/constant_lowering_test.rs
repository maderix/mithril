use mithril_front::core::{Core, CoreFn, CoreModule};

#[test]
fn literal_values_bypass_runtime_constructor_tag_limits() {
    for count in [0, 1, 199, 200, 201, 256] {
        let body = Core::Tuple((0..count).map(|cid| Core::Ctor(cid, vec![Core::Num(cid as i64 - 128)])).chain([Core::Flo(1.5), Core::Tuple(vec![])]).collect());
        let mut module = CoreModule::default();
        module.ctors = (0..count).map(|cid| (format!("C{cid}"), 1)).collect();
        module.fns.push(CoreFn { name: "main".into(), arity: 0, body, self_tail_rec: false, fold: None });
        let want = mithril_front::eval_core(&module, 0, &[]);
        let (program, lowered) = mithril_codegen::lower(&module);
        assert_eq!(program.constant, Some(want));
        assert_eq!(lowered.fns[0].body, module.fns[0].body);
        assert!(program.fns.is_empty() && program.rules.is_empty() && program.native_entries.is_empty());
        assert!(mithril_codegen::emit_rust(&module).contains("fn main()"));
    }
}

#[test]
fn lowering_is_independent_of_previous_modules_on_the_same_thread() {
    let sources = [
        "def a(x):\n    return x + 1\ndef f(n):\n    if n == 0:\n        return a(n)\n    return f(n - 1) + a(n)\ndef main():\n    return f(array_len(array_new(4, 0)))\n",
        "@data\nclass Tree:\n    Leaf: (x,)\n    Node: (a, b)\ndef f(n):\n    if n == 0:\n        return Leaf(n)\n    return Node(f(n - 1), f(n - 1))\ndef main():\n    return f(array_len(array_new(3, 0)))\n",
        "def g(x):\n    return x * 3\ndef f(n):\n    return g(n)\ndef main():\n    return f(array_len(array_new(7, 0)))\n",
    ];
    let modules:Vec<_>=sources.iter().map(|s|mithril_front::desugar(&mithril_front::parse(s).unwrap()).unwrap()).collect();
    let artifact=|m:&CoreModule| {
        let (p,lowered)=mithril_codegen::lower(m);
        let mut code=String::new();
        for f in &p.fns { mithril_codegen::lir::rust::func(f,&mut code); }
        (code,lowered)
    };
    let before:Vec<_>=modules.iter().map(&artifact).collect();
    for _ in 0..3 { for i in (0..modules.len()).rev() { assert_eq!(artifact(&modules[i]),before[i]); } }
}

#[test]
fn negative_literal_method_receivers_keep_their_sign() {
    use mithril_codegen::lir::{self, *};
    for n in [i64::MIN,-1099511627776,-1,0,1,i64::MAX] {
        let e=bin(Bop::Add,i64_(n),i64_(17));
        let printed=lir::rust::ex(&e);
        if n<0 {assert!(printed.starts_with(&format!("({n}i64).wrapping_add")),"{printed}");}
        else {assert!(printed.starts_with(&format!("{n}i64.wrapping_add")),"{printed}");}
    }
}
