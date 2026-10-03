use mithril_codegen::{emit_rust, fmt_val};
use mithril_front::{core::CoreModule, desugar, eval_core, parse};
use std::{path::Path, process::Command};

const TREE: &str = "@data\nclass Tree:\n    Leaf: (v,)\n    Pair: (l, r)\n    Offset: (t, n)\n\ndef build(d, x):\n    if d == 0:\n        return Leaf(x)\n    return Pair(build(d - 1, x - 7), Offset(build(d - 1, x + 11), d))\n\ndef chain(n):\n    if n == 0:\n        return Leaf(-1099511627776)\n    return Pair(chain(n - 1), Leaf(n))\n";
const WALK: &str = "def walk(t, x):\n    match t:\n        case Leaf(v):\n            return (v + x, 1)\n        case Pair(a, b):\n            l = walk(a, x)\n            r = walk(b, x)\n            return (l[0] * 3 - r[0], l[1] + r[1] + 1)\n        case Offset(a, n):\n            w = walk(a, x + n)\n            return (w[0] - n, w[1] + 1)\n";
const SUM: &str = "def walk(t, x):\n    match t:\n        case Leaf(v):\n            return v ^ x\n        case Pair(a, b):\n            return (walk(a, x) + walk(b, x)) & 4294967295\n        case Offset(a, n):\n            return walk(a, x + n) ^ n\n";

fn module(source: &str) -> CoreModule {
    desugar(&parse(source).unwrap()).unwrap()
}
fn source(body: &str, tree: &str, tuple: bool) -> String {
    let result = if tuple {
        "a[0] + b[0] + a[1] + b[1]"
    } else {
        "a + b"
    };
    format!("{TREE}\n{body}\ndef main():\n    n = array_len(array_new(3, 0))\n    t = {tree}\n    a = walk(t, -17)\n    b = walk(t, 4294967295)\n    return {result}\n")
}

#[test]
fn borrowed_recursive_walks_have_guarded_compact_continuations() {
    for (body, tuple) in [(SUM, false), (WALK, true)] {
        let (p, m) = mithril_codegen::lower(&module(&source(body, "build(n, 42)", tuple)));
        let fid = m.fns.iter().position(|f| f.name == "walk").unwrap();
        assert!(
            p.fns.iter().any(|f| f.name == format!("native_s_{fid}")),
            "borrowed recursion must use explicit frames"
        );
        assert!(
            p.fns.iter().any(|f| f.name == format!("region_d_{fid}")),
            "growth fallback must remain"
        );
        assert!(
            !p.native_entries.contains(&(fid as u32)),
            "heap handles are not integer-only task entries"
        );
        let code = emit_rust(&m);
        assert!(
            code.contains("native_ready(ctx, &[])"),
            "read-only walks use the surrounding WORK frontier"
        );
    }
    let body = SUM.replace("walk(a, x + n)", "walk(a, x)");
    let (p, m) = mithril_codegen::lower(&module(&source(&body, "build(n, 42)", false)));
    let fid = m.fns.iter().position(|f| f.name == "walk").unwrap();
    let native = p
        .fns
        .iter()
        .find(|f| f.name == format!("native_s_{fid}"))
        .unwrap();
    let mut reserves = 0;
    mithril_codegen::lir::walk_exprs(&native.body, &mut |e| {
        if matches!(e,mithril_codegen::lir::E::Call { f,.. } if f=="native_reserve") {
            reserves += 1;
        }
    });
    let mut records=Vec::new();
    mithril_codegen::lir::walk_stmts(&native.body,&mut |s| {
        if let mithril_codegen::lir::S::Let(_,mithril_codegen::lir::Ty::FrameRecords(k),_)=s {records.push(*k);}
    });
    assert!(reserves==1 || records==[2], "equivalent captures need one packed record layout: reserves={reserves},records={records:?}");
}

#[test]
fn borrowed_regions_match_rules_in_both_representations_and_execution_modes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(std::path::PathBuf::from).unwrap_or_else(||root.join("target")).join("release");
    let build = Command::new("cargo")
        .args(["build", "--release", "-p", "mithril-rt"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let dir = std::env::temp_dir().join(format!("mithril-borrowed-regions-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (case, body, tree, tuple) in [
        ("sum", SUM, "build(n, 42)", false),
        (
            "shared",
            &SUM.replace("walk(a, x + n)", "walk(a, x)"),
            "build(n, -1099511627776)",
            false,
        ),
        ("tuple", WALK, "build(n, -1099511627776)", true),
        ("deep", SUM, "chain(n * 27)", false),
    ] {
        let src = source(body, tree, tuple);
        let m = module(&src);
        let want = fmt_val(&eval_core(&m, m.main, &[]));
        let net_m = module(&src.replace("array_len(array_new(3, 0))", "3"));
        let mut net = mithril_net::build(&net_m);
        mithril_net::reduce(&mut net, &net_m, 1 << 20);
        assert_eq!(
            fmt_val(&mithril_net::readback(&net, mithril_net::root_port()).unwrap()),
            want
        );
        let fid = m.fns.iter().position(|f| f.name == "walk").unwrap();
        {
            let mut code = emit_rust(&m);
            let header = format!("fn s_{fid}(");
            let start = code.find(&header).expect("walk must have a scalar entry");
            let insert = start + code[start..].find("{\n").unwrap() + 2;
            code.insert_str(
                insert,
                "HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);\n",
            );
            let end = code.rfind("\n}").unwrap();
            code.insert_str(
                end,
                "\neprintln!(\"hits={}\", HITS.load(std::sync::atomic::Ordering::Relaxed));",
            );
            code.push_str("\nstatic HITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);\n");
            for mode in ["default", "grow", "native"] {
                let code = match mode {
                    "grow" => code.replace("if native_ready(ctx,", "if false && native_ready(ctx,"),
                    "native" => {
                        code.replace("if native_ready(ctx,", "if true || native_ready(ctx,")
                    }
                    _ => code.clone(),
                };
                std::fs::write(dir.join("main.rs"), code).unwrap();
                let compiled = Command::new("rustc")
                    .args(["--edition=2021", "-O"])
                    .arg(dir.join("main.rs"))
                    .arg("--extern")
                    .arg(format!(
                        "mithril_rt={}",
                        target.join("libmithril_rt.rlib").display()
                    ))
                    .arg("-L")
                    .arg(format!("dependency={}", target.join("deps").display()))
                    .arg("-o")
                    .arg(dir.join("run"))
                    .output()
                    .unwrap();
                assert!(
                    compiled.status.success(),
                    "{case}: {}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                for threads in ["1", "4"] {
                    for fuel in ["1", "64"] {
                        let got = Command::new(dir.join("run"))
                            .args([threads, fuel])
                            .output()
                            .unwrap();
                        assert!(
                            got.status.success(),
                            "{case} {mode} {threads} {fuel}: {}",
                            String::from_utf8_lossy(&got.stderr)
                        );
                        assert_eq!(
                            String::from_utf8_lossy(&got.stdout).trim(),
                            want,
                            "{case} {mode} {threads} {fuel}"
                        );
                        let err = String::from_utf8_lossy(&got.stderr);
                        let hits: usize = err
                            .lines()
                            .find_map(|l| l.strip_prefix("hits="))
                            .unwrap()
                            .parse()
                            .unwrap();
                        if mode == "grow" {
                            assert_eq!(hits, 0);
                        }
                        if mode == "native" || mode == "default" && threads == "1" {
                            assert!(hits > 0);
                        }
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
