use mithril_codegen::lir::{E, S};
use mithril_front::{core::CoreModule, desugar, eval_core, parse};
use std::{collections::BTreeSet, path::{Path, PathBuf}, process::Command};

const REGIONS: &str = "def tree(n):\n    if n < 2:\n        return 1\n    return tree(n - 1) + tree(n - 2)\n\ndef other(n):\n    if n < 2:\n        return (2, 3)\n    a = other(n - 1)\n    b = other(n - 2)\n    return (a[0] + b[0], a[1] + b[1])\n";
fn module(body: &str) -> CoreModule { desugar(&parse(body).unwrap()).unwrap() }
fn id(m: &CoreModule, name: &str) -> usize { m.fns.iter().position(|f| f.name == name).unwrap() }
fn calls(body: &[S]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    mithril_codegen::lir::walk_stmts(body, &mut |s| if let Some(e) = s.parts().0 {
        e.walk(&mut |e| if let E::Call { f, .. } = e { out.insert(f.clone()); });
    });
    out
}
fn guarded(p: &mithril_codegen::LirProgram, m: &CoreModule, name: &str, roots: &[&str]) {
    let fid = id(m, name);
    assert!(p.fns.iter().any(|f| f.name == format!("s_{fid}")), "{name} has no native form");
    let mut guards = 0;
    for prefix in ["d", "n"] {
        let Some(f) = p.fns.iter().find(|f| f.name == format!("{prefix}_{fid}")) else { continue };
        guards += 1;
        let f = if prefix == "d" && !matches!(f.body.first(), Some(S::If(..))) {
            let n = format!("n_{fid}");
            assert!(calls(&f.body).contains(&n), "{name}: tuple bridge must delegate to its guarded multi-value entry");
            p.fns.iter().find(|f| f.name == n).unwrap()
        } else { f };
        let S::If(E::Call { f: helper, args, .. }, fast, _) = &f.body[0] else { panic!("{name}: missing native-ready guard") };
        assert_eq!(helper, "native_ready");
        assert_eq!(args, &vec![E::Slice(roots.iter().map(|r| mithril_codegen::lir::u64_((1 + id(m, r)) as u64)).collect())]);
        assert!(calls(fast).contains(&format!("s_{fid}")), "{name}: native declaration is unused by guard");
        let fallback = format!("region_{}", f.name);
        assert!(calls(&f.body[1..]).contains(&fallback));
        assert!(p.fns.iter().any(|f| f.name == fallback), "{name}: growth fallback lost");
    }
    assert!(guards > 0, "{name}: guard check visited no function");
}

#[test]
fn acyclic_callers_keep_used_native_forms_and_all_downstream_region_guards() {
    let src = format!("{REGIONS}\ndef one(n):\n    return tree(n) + 9\n\ndef two(n):\n    return one(n) * 3\n\ndef choice(n):\n    if n % 2 == 0:\n        a = other(n)\n        return (a[0], tree(n))\n    return (tree(n), one(n))\n\ndef combine(n):\n    a = choice(n)\n    return a[0] + a[1] + two(n)\n\ndef main():\n    return combine(array_len(array_new(7, 0)))\n");
    let (p, m) = mithril_codegen::lower(&module(&src));
    guarded(&p, &m, "one", &["tree"]);
    guarded(&p, &m, "two", &["tree"]);
    guarded(&p, &m, "choice", &["tree", "other"]);
    guarded(&p, &m, "combine", &["tree", "other"]);
    let tuple = p.fns.iter().find(|f| f.name == format!("s_{}", id(&m, "choice"))).unwrap();
    assert_eq!(tuple.ret, mithril_codegen::lir::Ty::Tup(2));
}

#[test]
fn unsupported_values_calls_and_recursive_callers_remain_suspendable() {
    for body in [
        "def caller(n, a):\n    return tree(n) + array_len(a)\n\ndef main():\n    return caller(7, array_new(2, 0))\n",
        "def caller(n):\n    a = array_new(n, 0)\n    return tree(array_len(a))\n\ndef main():\n    return caller(7)\n",
        "def effect(n):\n    a = array_new(n, 0)\n    return array_len(a)\n\ndef caller(n):\n    return tree(n) + effect(n)\n\ndef main():\n    return caller(7)\n",
        "def caller(n):\n    return (tree(n), array_new(n, 0))\n\ndef main():\n    return caller(7)\n",
        "def caller(n):\n    return tree(n) + 0.5\n\ndef main():\n    return caller(7)\n",
        "def unknown(f, n):\n    return f(n)\n\ndef caller(n):\n    return unknown(lambda x: tree(x), n)\n\ndef main():\n    return caller(7)\n",
        "def caller(n):\n    if n < 2:\n        return tree(n)\n    return caller(n - 1)\n\ndef main():\n    return caller(7)\n",
    ] {
        let (p, m) = mithril_codegen::lower(&module(&format!("{REGIONS}\n{body}")));
        let fid = id(&m, "caller");
        assert!(!p.fns.iter().any(|f| f.name == format!("s_{fid}")), "unsupported caller became native: {body}");
        assert!(p.fns.iter().any(|f| f.name == format!("d_{fid}")));
    }
}

#[test]
fn guarded_native_and_growth_paths_match_rules_and_interpreter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    let build = Command::new("cargo").args(["build", "--release", "-p", "mithril-rt"]).current_dir(&root).output().unwrap();
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));
    let dir = std::env::temp_dir().join(format!("mithril-guarded-native-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in [
        ("tail", "def caller(n):\n    k = (n * 17 + 3) & 4294967295\n    if k % 2 == 0:\n        return tree(n - 1)\n    return tree(n)\n"),
        ("tail_tuple", "def caller(n):\n    if n == 0:\n        return (17, 29)\n    k = (n * 31 + 7) & 4294967295\n    if k % 2 == 0:\n        return other(n - 1)\n    return other(n)\n"),
        ("chain", "def one(n):\n    return tree(n) + 9\n\ndef caller(n):\n    return one(n) * 3\n"),
        ("tuple", "def caller(n):\n    if n % 2 == 0:\n        a = other(n)\n        return (a[0], tree(n))\n    return (tree(n), tree(n - 1))\n"),
        ("multiple", "def caller(n):\n    a = other(n)\n    return tree(n) + a[0] + a[1]\n"),
    ] {
        for n in [0, 6, 7] {
            let input = format!("array_len(array_new({n}, 0))");
            let src = format!("{REGIONS}\n{body}\ndef main():\n    return caller({input})\n");
            let m = module(&src); let want = mithril_codegen::fmt_val(&eval_core(&m, m.main, &[]));
            let net_m = module(&src.replace(&input, &n.to_string()));
            let mut net = mithril_net::build(&net_m);
            mithril_net::reduce(&mut net, &net_m, 1 << 20);
            assert_eq!(mithril_codegen::fmt_val(&mithril_net::readback(&net, mithril_net::root_port()).expect("rules must reach a value")), want);
            let fid = id(&m, "caller");
            {
                let original = mithril_codegen::emit_rust(&m);
                let start = original.find(&format!("fn s_{fid}(" )).expect("caller must have an actual native form");
                let insert = start + original[start..].find("\n").unwrap() + 1;
                let mut instrumented = original.clone();
                instrumented.insert_str(insert, "NATIVE_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);\n");
                let end = instrumented.rfind("\n}").unwrap();
                instrumented.insert_str(end, "\neprintln!(\"native_hits={}\", NATIVE_HITS.load(std::sync::atomic::Ordering::Relaxed));");
                instrumented.push_str("\nstatic NATIVE_HITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);\n");
                assert_eq!(instrumented.matches("NATIVE_HITS.fetch_add").count(), 1);
                assert!(instrumented.contains("if native_ready(ctx,"), "forced growth/native controls must rewrite actual guards");
                for (mode, code) in [("default", instrumented.clone()), ("grow", instrumented.replace("if native_ready(ctx,", "if false && native_ready(ctx,")), ("native", instrumented.replace("if native_ready(ctx,", "if true || native_ready(ctx,"))] {
                    std::fs::write(dir.join("main.rs"), &code).unwrap();
                    let c = Command::new("rustc").args(["--edition=2021", "-O"]).arg(dir.join("main.rs")).arg("--extern")
                        .arg(format!("mithril_rt={}", target.join("release/libmithril_rt.rlib").display())).arg("-L")
                        .arg(format!("dependency={}", target.join("release/deps").display())).arg("-o").arg(dir.join("run")).output().unwrap();
                    assert!(c.status.success(), "{name}: {}", String::from_utf8_lossy(&c.stderr));
                    for threads in ["1", "4"] { for fuel in ["1", "64"] {
                        let r = Command::new(dir.join("run")).args([threads, fuel]).output().unwrap();
                        assert!(r.status.success(), "{name} {mode}: {}", String::from_utf8_lossy(&r.stderr));
                        assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), want, "{name} n={n} {mode} threads={threads} fuel={fuel}");
                        let err = String::from_utf8_lossy(&r.stderr);
                        let hits: usize = err.lines().find_map(|l| l.strip_prefix("native_hits=")).unwrap().parse().unwrap();
                        if mode == "grow" { assert_eq!(hits, 0); }
                        if mode == "native" || mode == "default" && threads == "1" { assert!(hits > 0, "native declaration must execute"); }
                    } }
                }
            }
        }
    }
}

#[test]
fn tail_callers_share_one_integer_prefix_between_native_and_growth_forms() {
    let src = format!("{REGIONS}\ndef caller(n):\n    k = (n * 17 + 3) & 4294967295\n    if k % 2 == 0:\n        return tree(n - 1)\n    return tree(n)\n\ndef main():\n    return caller(array_len(array_new(7, 0)))\n");
    let (p, m) = mithril_codegen::lower(&module(&src)); let fid = id(&m, "caller");
    let prefix = format!("prefix_s_{fid}");
    assert_eq!(p.fns.iter().filter(|f| f.name == prefix).count(), 1, "tail-only integer prefix must be emitted once; growth={:?}", p.fns.iter().find(|f| f.name == format!("region_d_{fid}")));
    let scalar = p.fns.iter().find(|f| f.name == format!("s_{fid}")).unwrap();
    let growth = p.fns.iter().find(|f| f.name == format!("region_d_{fid}")).unwrap();
    assert!(calls(&scalar.body).contains(&prefix)); assert!(calls(&growth.body).contains(&prefix));
    for f in [scalar, growth] {
        let mut multiply = false;
        mithril_codegen::lir::walk_stmts(&f.body, &mut |s| if let Some(e) = s.parts().0 { e.walk(&mut |e| if matches!(e, E::Bin(mithril_codegen::lir::Bop::Mul, ..)) { multiply = true; }); });
        assert!(!multiply, "{0}: wrapper duplicated the shared arithmetic", f.name);
    }
    let src = src.replace("return tree(n)", "return tree(n) + k");
    let (p, m) = mithril_codegen::lower(&module(&src));
    assert!(!p.fns.iter().any(|f| f.name == format!("prefix_s_{}", id(&m, "caller"))), "post-call computation must decline prefix sharing");
}
