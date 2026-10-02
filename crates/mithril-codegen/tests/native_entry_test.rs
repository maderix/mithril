use mithril_front::{desugar,parse};
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn typed_launch_entries_require_a_pure_integer_call_region() {
    let _env = ENV_LOCK.lock().unwrap();
    std::env::set_var("MITHRIL_PLAIN_INTS","1");
    for (body,call,expected) in [
        ("def f(n):\n    if n == 0:\n        return 1\n    return f(n - 1) + n\n", "f(n)", true),
        ("def f(n, a):\n    if n == 0:\n        return array_get(a, 0)\n    return f(n - 1, a) + n\n", "f(n, array_new(1, 3))", false),
        ("def f(n):\n    if n == 0:\n        return 1\n    return f(n - 1) + array_len(array_new(n, 0))\n", "f(n)", false),
        ("def f(n):\n    return n + 1\n", "f(n)", false),
    ] {
        let src=format!("{body}\ndef main():\n    n = array_len(array_new(7, 0))\n    return {call}\n");
        let m=desugar(&parse(&src).unwrap()).unwrap();
        let (p,m)=mithril_codegen::lower(&m);
        let f=m.fns.iter().position(|f| f.name=="f").unwrap() as u32;
        assert_eq!(p.native_entries.contains(&f),expected,"{call}");
    }
    std::env::remove_var("MITHRIL_PLAIN_INTS");
}

#[test]
fn flattened_tuple_arguments_use_the_normal_call_bridge() {
    let _env = ENV_LOCK.lock().unwrap();
    std::env::set_var("MITHRIL_PLAIN_INTS", "1");
    for (params, body, call) in [
        ("n, p", "p[0] + p[1]", "f(n, (3, 7))"),
        ("n, p", "p[0] + p[1] + p[2]", "f(n, (3, 7, 11))"),
        ("n, p, q", "p[0] + p[1] + q[0] + q[1]", "f(n, (3, 7), (11, 13))"),
    ] {
        let extra = params.strip_prefix("n").unwrap();
        let src = format!("def f({params}):\n    if n == 0:\n        return {body}\n    return f(n - 1{extra}) + n\n\ndef main():\n    n = array_len(array_new(7, 0))\n    return {call}\n");
        let m = desugar(&parse(&src).unwrap()).unwrap();
        let (p, m) = mithril_codegen::lower(&m);
        let fid = m.fns.iter().position(|f| f.name == "f").unwrap() as u32;
        let native = p.fns.iter().find(|f| f.name == format!("s_{fid}")).unwrap();
        assert!(native.params.len() > m.fns[fid as usize].arity + 1, "{call} must exercise flattening");
        assert!(!p.native_entries.contains(&fid), "{call}: launch ABI has packed arguments, scalar function has flattened arguments");
    }
    std::env::remove_var("MITHRIL_PLAIN_INTS");
}
