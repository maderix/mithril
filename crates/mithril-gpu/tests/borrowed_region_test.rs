use mithril_rt::Redex;

#[test]
#[ignore = "requires CUDA and nvcc; changes native cache settings"]
fn borrowed_walks_preserve_sharing_handles_and_cache_spills() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tree = std::fs::read_to_string(
        root.join("crates/mithril-codegen/tests/fixtures/borrowed_tree.py"),
    )
    .unwrap();
    let interpreter = std::fs::read_to_string(root.join("crates/mithril-codegen/tests/fixtures/recursive_expr.py")).unwrap();
    let deep_walk=interpreter.split("def main():").next().unwrap().to_string()+
        "def chain(n):\n    if n == 0:\n        return Lit(-1099511627776)\n    return Add(chain(n - 1), Sub(Lit(n), Var()))\n\ndef main():\n    t = chain(array_len(array_new(132, 0)))\n    return eval(t, -17) + eval(t, 4294967295)\n";
    let cases = [
        ("tuple", tree.clone()),
        (
            "deep",
            tree.replace("build(n, -1099511627776)", "chain(n * 22)"),
        ),
        ("interpreter", interpreter),
        ("joined", deep_walk),
    ];
    let cache = root.join("target/borrowed-oracles");
    for (name, source) in cases {
        let m = mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
        let want = mithril_codegen::fmt_val(&mithril_front::eval_core(&m, m.main, &[]));
        let (p, lowered) = mithril_codegen::lower(&m);
        let fid = lowered
            .fns
            .iter()
            .position(|f| {
                f.name
                    == if matches!(name, "interpreter" | "joined") {
                        "eval"
                    } else {
                        "walk"
                    }
            })
            .unwrap();
        assert!(p.fns.iter().any(|f| f.name == format!("native_s_{fid}")));
        let cu = mithril_gpu::emit_cuda(&m).unwrap();
        let bridge = p.fns.iter().find(|f| f.name == format!("d_{fid}")).unwrap();
        assert!(
            matches!(bridge.body.first(),Some(mithril_codegen::lir::S::If(mithril_codegen::lir::E::Call { f,args,.. },_,_)) if f=="native_ready" && args==&vec![mithril_codegen::lir::E::Slice(vec![])]),
            "borrowed guard must use its owning WORK task"
        );
        for native in [false, true] {
            let code = cu.replace(
                "if (native_ready(",
                if native {
                    "if (true || native_ready("
                } else {
                    "if (false && native_ready("
                },
            );
            for words in ["0", "1", "auto"] {
                std::env::set_var("MITHRIL_GPU_NATIVE_WORDS", words);
                let got = mithril_gpu::compile_and_run(&code, Redex::default(), &cache).unwrap();
                assert_eq!(got.text, want, "{name}: native={native} cache={words}");
            }
        }
        std::env::set_var("MITHRIL_GPU_NATIVE_WORDS", "auto");
        if name == "joined" {
            assert!(
                cu.contains("read_pair("),
                "shared field primitive must be emitted"
            );
            let poisoned=cu.replace("read_pair(","poisoned_pair(")
                .replace("#include \"engine.cu\"", "#include \"engine.cu\"\n__device__ A<2> poisoned_pair(u64 p) { g_abort(4); return read_pair(p); }")
                .replace("if (native_ready(", "if (true || native_ready(");
            let error = mithril_gpu::compile_and_run(&poisoned, Redex::default(), &cache)
                .expect_err("borrowed cell primitive must execute");
            assert!(error.contains("unsupported device feature"), "{error}");
        }
        let start = cu.rfind(&format!(" native_s_{fid}(")).unwrap();
        let insert = start + cu[start..].find("{\n").unwrap() + 2;
        let mut poisoned = cu.clone();
        poisoned.insert_str(insert, "g_abort(4);\n");
        poisoned = poisoned.replace("if (native_ready(", "if (true || native_ready(");
        let error = mithril_gpu::compile_and_run(&poisoned, Redex::default(), &cache)
            .expect_err("native traversal must execute");
        assert!(
            error.contains("unsupported device feature"),
            "{name}: {error}"
        );
    }
    std::env::remove_var("MITHRIL_GPU_NATIVE_WORDS");
}
