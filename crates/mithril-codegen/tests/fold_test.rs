//! Certified summaries must agree with both net reduction and native execution.
use mithril_front::{desugar,parse,eval_core};
use mithril_codegen::{emit_rust_opts,EmitOpts,fmt_val};
use std::process::Command;

#[test]
fn recursive_summaries_preserve_counts_and_wrapping_seeds() {
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source=std::fs::read_to_string(root.join("crates/mithril-codegen/tests/fixtures/recursive_counts.py")).unwrap();
    let solve=source.split("def main():").next().unwrap();
    let dir=std::env::temp_dir().join(format!("mithril-fold-{}",std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let built=Command::new("cargo").args(["build","--release","-p","mithril-rt"]).current_dir(&root).output().unwrap();
    assert!(built.status.success(),"{}",String::from_utf8_lossy(&built.stderr));
    for (n,solutions,nodes) in [(0,1u32,0u32),(1,2,2),(4,16,30),(5,32,62),(6,64,126),(8,256,510)] {
        for seed in [0u32,u32::MAX] {
            let call=format!("solve(n, 2, 0, 1, 0, {seed}, {seed})");
            let constant=format!("{solve}\ndef main():\n    n = {n}\n    return {call}\n");
            let m=desugar(&parse(&constant).unwrap()).unwrap();
            let want=format!("({}, {})",solutions.wrapping_add(seed),nodes.wrapping_add(seed));
            assert_eq!(fmt_val(&eval_core(&m,m.main,&[])),want);
            let mut net=mithril_net::build(&m);
            mithril_net::reduce(&mut net,&m,5_000_000);
            assert_eq!(mithril_net::readback(&net,mithril_net::root_port()).map(|v| fmt_val(&v)),Some(want.clone()),"net: n={n} seed={seed}");
            let runtime=constant.replace(&format!("n = {n}"),&format!("n = array_len(array_new({n}, 0))"));
            let m=desugar(&parse(&runtime).unwrap()).unwrap();
            for rep in [Some(false),Some(true)] {
                let code=emit_rust_opts(&m,EmitOpts { int_rep:rep });
                std::fs::write(dir.join("main.rs"),code).unwrap();
                let c=Command::new("rustc").args(["--edition=2021","-O"]).arg(dir.join("main.rs"))
                    .arg("--extern").arg(format!("mithril_rt={}",root.join("target/release/libmithril_rt.rlib").display()))
                    .arg("-L").arg(format!("dependency={}",root.join("target/release/deps").display()))
                    .arg("-o").arg(dir.join("run")).output().unwrap();
                assert!(c.status.success(),"{}",String::from_utf8_lossy(&c.stderr));
                for threads in ["1","4"] { for fuel in ["1","64"] {
                    let r=Command::new(dir.join("run")).args([threads,fuel]).output().unwrap();
                    assert!(r.status.success(),"{}",String::from_utf8_lossy(&r.stderr));
                    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(),want,"n={n} seed={seed} rep={rep:?} threads={threads} fuel={fuel}");
                } }
            }
        }
    }
}

#[test]
fn control_that_depends_on_seed_high_bits_keeps_original_execution() {
    let body = "def f(n, a, b):\n    while n > 0:\n        if a * 4294967296 == 0:\n            b = (b + 1) & 4294967295\n        w = f(n - 1, (a + 1) & 4294967295, b)\n        a = w[0]\n        b = w[1]\n        n = n - 1\n    return (a, b)\n";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("mithril-fold-control-{}",std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let built = Command::new("cargo").args(["build","--release","-p","mithril-rt"]).current_dir(&root).output().unwrap();
    assert!(built.status.success());
    for seed in [0u32,1,u32::MAX] {
        let constant = format!("{body}\ndef main():\n    return f(4, {seed}, 0)\n");
        let m = desugar(&parse(&constant).unwrap()).unwrap();
        let want = fmt_val(&eval_core(&m,m.main,&[]));
        let mut net = mithril_net::build(&m);
        mithril_net::reduce(&mut net,&m,5_000_000);
        assert_eq!(mithril_net::readback(&net,mithril_net::root_port()).map(|v|fmt_val(&v)),Some(want.clone()));
        let source = constant.replace("return f(4,", "return f(array_len(array_new(4, 0)),");
        let m = desugar(&parse(&source).unwrap()).unwrap();
        for rep in [Some(false),Some(true)] {
            std::fs::write(dir.join("main.rs"),emit_rust_opts(&m,EmitOpts { int_rep:rep })).unwrap();
            let c = Command::new("rustc").args(["--edition=2021","-O"]).arg(dir.join("main.rs"))
                .arg("--extern").arg(format!("mithril_rt={}",root.join("target/release/libmithril_rt.rlib").display()))
                .arg("-L").arg(format!("dependency={}",root.join("target/release/deps").display()))
                .arg("-o").arg(dir.join("run")).output().unwrap();
            assert!(c.status.success(),"{}",String::from_utf8_lossy(&c.stderr));
            for threads in ["1","4"] { for fuel in ["1","64"] {
                let r = Command::new(dir.join("run")).args([threads,fuel]).output().unwrap();
                assert!(r.status.success());
                assert_eq!(String::from_utf8_lossy(&r.stdout).trim(),want,"seed={seed} rep={rep:?} threads={threads} fuel={fuel}");
            } }
        }
    }
}
