use mithril_codegen::fmt_val;
use mithril_front::{desugar, eval_core, parse};

#[test]
fn recursive_native_regions_have_explicit_continuations() {
    let src = "def count(n):\n    if n == 0:\n        return 1\n    return count(n - 1) * 3 + n\n\ndef main():\n    return count(array_len(array_new(12, 0)))\n";
    let m = desugar(&parse(src).unwrap()).unwrap();
    run_native(src, "count");
    assert_eq!(fmt_val(&eval_core(&m, m.main, &[])), "930015");
}

fn run_native(src: &str, name: &str) {
    use std::process::Command;
    let module = desugar(&parse(src).unwrap()).unwrap();
    let want = fmt_val(&eval_core(&module, module.main, &[]));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("target"));
    let build = Command::new("cargo").args(["build", "--release", "-p", "mithril-rt"]).current_dir(&root).output().unwrap();
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));
    let dir = std::env::temp_dir().join(format!("mithril-native-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    {
        let code = mithril_codegen::emit_rust(&module);
        std::fs::write(dir.join("main.rs"), code).unwrap();
        let c = Command::new("rustc").args(["--edition=2021", "-O"]).arg(dir.join("main.rs"))
            .arg("--extern").arg(format!("mithril_rt={}", target.join("release/libmithril_rt.rlib").display()))
            .arg("-L").arg(format!("dependency={}", target.join("release/deps").display()))
            .arg("-o").arg(dir.join("run")).output().unwrap();
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
        for threads in ["1", "4"] {
            for fuel in ["1", "64"] {
                let r = Command::new(dir.join("run")).args([threads, fuel]).output().unwrap();
                assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
                assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), want, "{name}: threads={threads} fuel={fuel}");
            }
        }
    }
}

#[test]
fn return_frames_preserve_nested_calls_and_live_values() {
    for (name, body) in [
        ("recursive_fold", "def g(n, a, b):\n    while n > 0:\n        w = g(n - 1, a, (b + 1) & 4294967295)\n        a = w[0]\n        b = w[1]\n        n = n - 1\n    return (a, b)\n\ndef f(n):\n    return g(n, 4294967295, 0)\n"),
        ("constant", "def f(n):\n    if n == 0:\n        return 1\n    return f(n - 1) * 3\n"),
        ("wide_capture", "def g(n, x):\n    if n == 0:\n        return x\n    y = g(n - 1, x + n)\n    return y - x\n\ndef f(n):\n    return g(n, -1099511627776)\n"),
        ("wide", "def f(n):\n    if n == 0:\n        return -1099511627776\n    return f(n - 1) + 1099511627777\n"),
        ("chain", "def f(n):\n    if n == 0:\n        return 1\n    return (f(n - 1) * 3 + n) & 4294967295\n"),
        ("nested", "def g(n):\n    if n == 0:\n        return 1\n    return g(n - 1) + n\n\ndef f(n):\n    if n == 0:\n        return 0\n    x = f(n - 1)\n    return x + g(n)\n"),
        ("mutual", "def g(n):\n    if n == 0:\n        return (2, 3, 5)\n    x = f(n - 1)\n    return (x, x + n, n)\n\ndef f(n):\n    if n == 0:\n        return 7\n    x = g(n - 1)\n    return x[2] + x[0] * 3 + x[1]\n"),
        ("loop", "def f(n):\n    s = n\n    for i in range(n):\n        s = s + f(i)\n    return s\n"),
    ] {
        run_native(&format!("{body}\ndef main():\n    return f(array_len(array_new(7, 0)))\n"), name);
    }
}
