use mithril_front::{desugar, parse, eval_core};
use mithril_codegen::{emit_rust, fmt_val};
use std::process::Command;

#[test]
fn native_and_suspended_regions_match_for_wide_and_thin_frontiers() {
    let cases = [
        ("tree", "def f(n):\n    if n == 0:\n        return (1, 0)\n    a = f(n - 1)\n    b = f(n - 1)\n    return (a[0] + b[0], a[1] + b[1] + 1)\n", "f(n)"),
        ("wide", "def f(n, x):\n    if n == 0:\n        return x\n    return f(n - 1, x - n) + f(n - 1, x + n)\n", "f(n, -1099511627776)"),
        ("outer", "def g(n):\n    if n == 0:\n        return 1\n    return g(n - 1) + g(n - 1)\n\ndef f(n):\n    if n == 0:\n        return g(5)\n    return f(n - 1) + f(n - 1)\n", "f(n)"),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let build = Command::new("cargo").args(["build", "--release", "-p", "mithril-rt"]).current_dir(&root).output().unwrap();
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));
    let dir = std::env::temp_dir().join(format!("mithril-regions-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = root.join("target/release");
    for (name, body, call) in cases {
        let source = format!("{body}\ndef main():\n    n = array_len(array_new(7, 0))\n    return {call}\n");
        let module = desugar(&parse(&source).unwrap()).unwrap();
        let want = fmt_val(&eval_core(&module, module.main, &[]));
        {
            let code = emit_rust(&module);
            std::fs::write(dir.join("main.rs"), code).unwrap();
            let c = Command::new("rustc").args(["--edition=2021", "-O"]).arg(dir.join("main.rs"))
                .arg("--extern").arg(format!("mithril_rt={}", target.join("libmithril_rt.rlib").display()))
                .arg("-L").arg(format!("dependency={}", target.join("deps").display()))
                .arg("-o").arg(dir.join("run")).output().unwrap();
            assert!(c.status.success(), "{name}: {}", String::from_utf8_lossy(&c.stderr));
            for threads in ["1", "4"] { for fuel in ["1", "64"] {
                let r = Command::new(dir.join("run")).args([threads, fuel]).output().unwrap();
                assert!(r.status.success(), "{name}: {}", String::from_utf8_lossy(&r.stderr));
                assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), want, "{name}: threads={threads} fuel={fuel}");
            } }
        }
    }
}
