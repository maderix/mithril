use mithril_codegen::lir::{self, *, completed};

// These two lowering probes override one process-wide representation setting.
static REP_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn function(name: &str, body: Vec<S>) -> FnDef {
    FnDef { name: name.into(), ctx: false, params: vec![("budget".into(), Ty::RefI64), ("n".into(), Ty::I64)], ret: Ty::I64, body, inline: Inline::Default, cold: false }
}
fn leaf() -> FnDef {
    function("leaf", vec![let_("used", Ty::I64, i64_(0)), set("used", bin(Bop::Add, v("used"), i64_(1))), do_(p("native_work_fuel", vec![v("budget"), v("used")])), ret(bin(Bop::Add, v("n"), i64_(1)))])
}
fn variants(fns: &[FnDef]) -> (std::collections::BTreeMap<String, String>, Vec<FnDef>) {
    completed::entries(fns, &["entry".into()])
}
#[test]
fn completed_closure_has_no_credit_but_the_original_keeps_its_exact_charges() {
    let entry = function("entry", vec![ret(p("leaf", vec![v("budget"), v("n")]))]);
    let fns = vec![entry, leaf()];
    let original = fns.clone();
    let (roots, clones) = variants(&fns);
    assert_eq!(roots.len(), 1, "a pure integer closure must get a completed-task entry");
    assert_eq!(clones.len(), 2);
    assert_eq!(fns, original);
    for f in &clones {
        assert!(!f.params.iter().any(|(_, t)| *t == Ty::RefI64));
        let mut text = String::new(); lir::rust::func(f, &mut text);
        assert!(!text.contains("budget") && !text.contains("used"));
    }
    let root = clones.iter().find(|f| f.name == roots["entry"]).unwrap();
    let S::Ret(E::Call { f, args, .. }) = &root.body[0] else { panic!("root must call its cloned child") };
    assert_ne!(f, "leaf");
    assert_eq!(args, &vec![v("n")]);
}
#[test]
fn observing_the_budget_or_charge_counter_rejects_the_whole_closure() {
    for observed in [E::Deref("budget".into()), E::Ref("budget".into()), E::Addr("budget".into()), v("used")] {
        let mut child = leaf(); child.body.push(ret(observed));
        let entry = function("entry", vec![ret(p("leaf", vec![v("budget"), v("n")]))]);
        assert!(variants(&[entry, child]).0.is_empty());
    }
}
#[test]
fn effects_unknown_helpers_and_transitive_context_are_rejected() {
    for effect in [c("anything", vec![]), p("arr_len_of", vec![v("n")]), p("atomic_load", vec![v("n")]), p("unknown", vec![v("n")]), p("fuel_of", vec![v("budget")])] {
        let mut child = leaf(); child.body.insert(0, do_(effect));
        let entry = function("entry", vec![ret(p("leaf", vec![v("budget"), v("n")]))]);
        assert!(variants(&[entry, child]).0.is_empty());
    }
    let mut child = leaf(); child.ctx = true;
    assert!(variants(&[function("entry", vec![ret(p("leaf", vec![v("budget"), v("n")]))]), child]).0.is_empty());
}

#[test]
fn writes_and_aliases_of_the_budget_are_observations() {
    for observation in [set("budget", i64_(9)), S::Decl("budget".into(), Ty::I64), let_("alias", Ty::RefI64, v("budget")), S::Store("budget".into(), i64_(9))] {
        let mut f = leaf(); f.name = "entry".into(); f.body.insert(0, observation);
        assert!(variants(&[f]).0.is_empty());
    }
}

#[test]
fn branch_cycles_are_checked_and_shared_callees_are_cloned_once() {
    let mut child = leaf();
    child.body.insert(2, S::If(bin(Bop::Gt, v("n"), i64_(0)), vec![do_(p("leaf", vec![v("budget"), bin(Bop::Sub, v("n"), i64_(1))]))], vec![]));
    let entry = function("entry", vec![ret(p("leaf", vec![v("budget"), v("n")]))]);
    let mut other = entry.clone(); other.name = "other".into();
    let (roots, clones) = completed::entries(&[entry.clone(), other, child.clone()], &["entry".into(), "other".into(), "absent".into()]);
    assert_eq!(roots.len(), 2); assert_eq!(clones.len(), 3);
    child.body.insert(0, S::Switch(v("n"), vec![(0, vec![do_(p("unknown", vec![]))])], Some(vec![])));
    assert!(variants(&[entry, child]).0.is_empty(), "one unverified branch rejects the transitive closure");
}

#[test]
fn credited_and_completed_native_code_match_the_interpreter() {
    let _rep = REP_ENV.lock().unwrap();
    use std::{path::{Path, PathBuf}, process::Command};
    std::env::set_var("MITHRIL_PLAIN_INTS", "1");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    let build = Command::new("cargo").args(["build", "--release", "-p", "mithril-rt"]).current_dir(&root).output().unwrap();
    assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stderr));
    let dir = std::env::temp_dir().join(format!("mithril-completed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cases = [
        ("chain", "def f(n):\n    if n == 0:\n        return 1\n    return (f(n - 1) * 31 + n) & 4294967295\n", "n"),
        ("wide", "def f(n, x):\n    if n == 0:\n        return x\n    y = f(n - 1, x + n)\n    return y - x\n", "n, -1099511627776"),
        ("tuple", "def f(n, a, b, c):\n    if n == 0:\n        return (a, b, c)\n    w = f(n - 1, a, b, c)\n    return ((w[0] + n) & 4294967295, (w[1] * 3) & 4294967295, (w[2] - n) & 4294967295)\n", "n, 4294967295, 19, 7"),
        ("nested", "def g(n):\n    if n == 0:\n        return 1\n    return g(n - 1) + n\n\ndef f(n):\n    if n == 0:\n        return 0\n    return f(n - 1) + g(n)\n", "n"),
    ];
    for (name, body, args) in cases {
        for n in [0, 1, 4, 19] {
            let src = format!("{body}\ndef main():\n    n = array_len(array_new({n}, 0))\n    return f({args})\n");
            let module = mithril_front::desugar(&mithril_front::parse(&src).unwrap()).unwrap();
            let want = mithril_codegen::fmt_val(&mithril_front::eval_core(&module, module.main, &[]));
            let (program, module) = mithril_codegen::lower(&module);
            let fid = module.fns.iter().position(|f| f.name == "f").unwrap() as u32;
            assert!(program.native_entries.contains(&fid), "{name}: expected native entry");
            let entry = format!("s_{fid}");
            let original = program.fns.clone();
            let (roots, clones) = completed::entries(&program.fns, &[entry.clone()]);
            assert!(roots.contains_key(&entry), "{name}: pure integer entry rejected");
            assert_eq!(program.fns, original);
            let mut code = "#![allow(unused, unused_mut, unreachable_code, non_snake_case)]\nuse mithril_rt::prelude::*;\n".to_string();
            for clone in &clones {
                let source = program.fns.iter().find(|f| clone.name == format!("value_{}", f.name)).unwrap();
                lir::rust::func(source, &mut code); lir::rust::func(clone, &mut code);
            }
            let result = if matches!(program.fns.iter().find(|f| f.name == entry).unwrap().ret, Ty::I64) { "format!(\"{}\", value)" } else { "format!(\"{:?}\", value)" };
            code.push_str(&format!("fn main() {{ let n={n}i64; for budget in [1i64, 64, i64::MAX] {{ let mut fuel=budget; let value={entry}(&mut fuel,{args}); let credited={result}; assert!(fuel<budget,\"ordinary entry lost its charges\"); let value={}({args}); assert_eq!(credited,{result}); println!(\"{{}}\",credited); }} }}", roots[&entry]));
            std::fs::write(dir.join("main.rs"), code).unwrap();
            let compile = Command::new("rustc").args(["--edition=2021", "-O"]).arg(dir.join("main.rs")).arg("--extern")
                .arg(format!("mithril_rt={}", target.join("release/libmithril_rt.rlib").display())).arg("-L")
                .arg(format!("dependency={}", target.join("release/deps").display())).arg("-o").arg(dir.join("run")).output().unwrap();
            assert!(compile.status.success(), "{name}: {}", String::from_utf8_lossy(&compile.stderr));
            let run = Command::new(dir.join("run")).output().unwrap();
            assert!(run.status.success(), "{name}: {}", String::from_utf8_lossy(&run.stderr));
            assert_eq!(String::from_utf8_lossy(&run.stdout).lines().collect::<Vec<_>>(), vec![want.as_str(); 3], "{name} n={n}");
        }
    }
    std::env::remove_var("MITHRIL_PLAIN_INTS");
}

#[test]
fn erased_cost_computation_must_be_total_and_have_no_effects() {
    for amount in [p("unknown", vec![]), bin(Bop::Div, i64_(1), i64_(0)), idx(E::Arr(vec![]), 0), E::Neg(Box::new(v("n")))] {
        let mut f = leaf(); f.name = "entry".into();
        f.body[1] = set("used", bin(Bop::Add, v("used"), amount));
        assert!(variants(&[f]).0.is_empty(), "erasing a charge must not erase a trap or effect");
    }
    let mut f = leaf(); f.name = "entry".into();
    f.body[2] = do_(p("native_work_fuel", vec![v("budget"), E::Neg(Box::new(v("n")))]));
    assert!(variants(&[f]).0.is_empty(), "direct negation can overflow for n=i64::MIN");
}

#[test]
fn recursive_fold_entry_can_complete_without_budget_observation() {
    let _rep = REP_ENV.lock().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(root.join("crates/mithril-codegen/tests/fixtures/recursive_counts.py")).unwrap();
    std::env::set_var("MITHRIL_PLAIN_INTS", "1");
    let module = mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
    let (program, _) = mithril_codegen::lower(&module);
    assert!(!program.native_entries.is_empty());
    let roots = program.native_entries.iter().map(|fid| format!("s_{fid}")).collect::<Vec<_>>();
    let (entries, clones) = completed::entries(&program.fns, &roots);
    assert_eq!(entries.len(), roots.len(), "existing pure integer native entries must receive completed-task variants");
    assert!(!clones.is_empty());
    std::env::remove_var("MITHRIL_PLAIN_INTS");
}

#[test]
fn private_frame_references_cannot_escape_as_program_values() {
    for escaped in [E::Ref("frames".into()), E::Addr("frames".into()), E::Deref("frames".into()), v("frames")] {
        let mut f = leaf(); f.name = "entry".into();
        f.body.insert(0, let_("frames", Ty::Frames, p("native_frames", vec![u32_(0)])));
        f.body.push(ret(escaped));
        assert!(variants(&[f]).0.is_empty());
    }
}

#[test]
fn accumulated_charges_accept_only_total_independent_addends() {
    for addend in [i64_(1), v("n"), bin(Bop::Mul, v("n"), i64_(7))] {
        let mut f = leaf(); f.name = "entry".into();
        f.body[1] = set("used", bin(Bop::Add, bin(Bop::Add, v("used"), i64_(1)), addend));
        let (entries, clones) = variants(&[f]);
        assert_eq!(entries.len(), 1);
        assert_eq!(clones[0].body, vec![ret(bin(Bop::Add, v("n"), i64_(1)))]);
    }
    for addend in [v("budget"), v("used"), E::Deref("budget".into()), p("unknown", vec![]), bin(Bop::Div, i64_(1), i64_(0)), E::Neg(Box::new(v("n")))] {
        let mut f = leaf(); f.name = "entry".into();
        f.body[1] = set("used", bin(Bop::Add, bin(Bop::Add, v("used"), i64_(1)), addend));
        assert!(variants(&[f]).0.is_empty(), "unsafe nested increment must reject");
    }
}

#[test]
fn grouped_frame_values_cannot_hide_budget_observations_or_escape_storage() {
    for value in [E::Ref("budget".into()),E::Addr("frames".into()),v("used")] {
        let mut f=leaf();f.name="entry".into();
        f.body.insert(0,let_("frames",Ty::FrameRecords(2),p("native_records",vec![E::Arr(vec![u64_(0);2])])));
        f.body.insert(1,do_(p("native_record_push",vec![E::Ref("frames".into()),E::Arr(vec![value,u64_(0)])])));
        assert!(variants(&[f]).0.is_empty());
    }
    let mut f=leaf();f.name="entry".into();
    f.body[1]=set("used",bin(Bop::Add,v("used"),idx(E::Arr(vec![i64_(1)]),0)));
    assert!(variants(&[f]).0.is_empty(),"retained array access is allowed; erased charge indexing is not total");
}
