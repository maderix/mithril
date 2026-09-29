//! Task 4 tests: fold detection over the surface AST, associativity +
//! identity proving over Z_2^56, AST marking + desugar carry-through,
//! sequential (eval_core) semantics preservation, and Lean obligations.

use mithril_front::ast::{Combiner, FoldInfo, Module, Stmt};
use mithril_front::core::Combiner as CoreCombiner;
use mithril_front::{desugar, eval_core, CoreModule, Val};
use mithril_reassoc::{analyze, lean_obligations, FoldReport};

// ---- helpers ----

fn analyzed(src: &str) -> (Module, Vec<FoldReport>) {
    let mut m = mithril_front::parse(src).expect("parse failed");
    let reports = analyze(&mut m);
    (m, reports)
}

fn report<'r>(reports: &'r [FoldReport], func: &str) -> &'r FoldReport {
    reports
        .iter()
        .find(|r| r.func == func)
        .unwrap_or_else(|| panic!("no fold report for fn '{func}' in {reports:?}"))
}

/// The `fold` field of the first top-level `for` in `func`.
fn fold_of<'m>(m: &'m Module, func: &str) -> &'m Option<FoldInfo> {
    let f = m.fns.iter().find(|f| f.name == func).unwrap_or_else(|| panic!("no fn '{func}'"));
    f.body
        .iter()
        .find_map(|s| match s {
            Stmt::For(_, _, _, fold) => Some(fold),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no top-level for loop in '{func}'"))
}

fn fn_id(cm: &CoreModule, name: &str) -> u32 {
    cm.fns.iter().position(|f| f.name == name).unwrap_or_else(|| panic!("no core fn '{name}'")) as u32
}

// ---- sources ----

const SRC_SUM: &str = "\
def f(x):
    return x*x + 1

def total(n):
    s = 0
    for i in range(n):
        s = s + f(i)
    return s
";

const SRC_HIST: &str = "\
def hadd(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4], a[5] + b[5], a[6] + b[6], a[7] + b[7])

def contrib(i):
    return (i, i*i, 1, 0, 0, 0, 0, 0)

def hist(n):
    h = (0, 0, 0, 0, 0, 0, 0, 0)
    for i in range(n):
        h = hadd(h, contrib(i))
    return h
";

const SRC_GOLDEN: &str = "\
def hadd(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4], a[5] + b[5], a[6] + b[6], a[7] + b[7])

def contrib(i):
    return (i, i*i, 1, 0, 0, 0, 0, 0)

def hist(n):
    h = (0, 0, 0, 0, 0, 0, 0, 0)
    for i in range(n):
        h = hadd(h, contrib(i))
    return h

def recolor(n):
    s = 0
    for i in range(n):
        s = s + i*3
    return s
";

// ---- proven folds ----

#[test]
fn fold_scalar_sum_proven() {
    let (m, reps) = analyzed(SRC_SUM);
    let r = report(&reps, "total");
    assert!(r.proven, "expected proven, got: {}", r.reason);
    assert_eq!(r.acc, "s");
    assert_eq!((r.arity, r.bits), (1, 56));
    assert_eq!(fold_of(&m, "total"), &Some(FoldInfo { combiner: Combiner::WrapAdd, proven: true }));
}

#[test]
fn fold_tuple_hist_proven() {
    let (m, reps) = analyzed(SRC_HIST);
    let r = report(&reps, "hist");
    assert!(r.proven, "expected proven, got: {}", r.reason);
    assert_eq!(r.acc, "h");
    assert_eq!((r.arity, r.bits), (8, 56));
    assert_eq!(
        fold_of(&m, "hist"),
        &Some(FoldInfo { combiner: Combiner::TupleWrapAdd(8), proven: true })
    );
    // desugar carries the marking onto the generated fold helper fn,
    // and the sequential oracle semantics are unchanged by the marking.
    let cm = desugar(&m).expect("desugar failed");
    let helper = cm.fns.iter().find(|f| f.fold.is_some()).expect("no CoreFn carries fold info");
    let fi = helper.fold.as_ref().unwrap();
    assert!(fi.proven);
    assert_eq!(fi.combiner, CoreCombiner::TupleWrapAdd(8));
    let v = eval_core(&cm, fn_id(&cm, "hist"), &[Val::I(3)]);
    let want = Val::T(std::sync::Arc::new(vec![
        Val::I(3), // 0+1+2
        Val::I(5), // 0+1+4
        Val::I(3), // 1+1+1
        Val::I(0),
        Val::I(0),
        Val::I(0),
        Val::I(0),
        Val::I(0),
    ]));
    assert_eq!(v, want);
}

// ---- masked (u32-emulation) folds: `& 4294967295` => Z_2^32 mode ----

#[test]
fn fold_masked_scalar_proven() {
    let src = "\
def f(x):
    return x*x + 1

def total32(n):
    s = 0
    for i in range(n):
        s = (s + f(i)) & 4294967295
    return s
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "total32");
    assert!(r.proven, "expected proven, got: {}", r.reason);
    assert_eq!((r.arity, r.bits), (1, 32));
    assert_eq!(
        fold_of(&m, "total32"),
        &Some(FoldInfo { combiner: Combiner::WrapAdd32, proven: true })
    );
    // desugar carry-through + sequential oracle unchanged by the marking
    let cm = desugar(&m).expect("desugar failed");
    let helper = cm.fns.iter().find(|f| f.fold.is_some()).expect("no CoreFn carries fold info");
    assert_eq!(helper.fold.as_ref().unwrap().combiner, CoreCombiner::WrapAdd32);
    assert_eq!(eval_core(&cm, fn_id(&cm, "total32"), &[Val::I(3)]), Val::I(8)); // 1+2+5
    assert_eq!(eval_core(&cm, fn_id(&cm, "total32"), &[Val::I(0)]), Val::I(0));
}

#[test]
fn fold_masked_tuple_proven() {
    let src = "\
def hadd32(a, b):
    return ((a[0] + b[0]) & 4294967295, (a[1] + b[1]) & 4294967295)

def hist32(n):
    h = (0, 0)
    for i in range(n):
        h = hadd32(h, (i, i*i))
    return h
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "hist32");
    assert!(r.proven, "expected proven, got: {}", r.reason);
    assert_eq!((r.arity, r.bits), (2, 32));
    assert_eq!(
        fold_of(&m, "hist32"),
        &Some(FoldInfo { combiner: Combiner::TupleWrapAdd32(2), proven: true })
    );
    let cm = desugar(&m).expect("desugar failed");
    let v = eval_core(&cm, fn_id(&cm, "hist32"), &[Val::I(3)]);
    assert_eq!(v, Val::T(std::sync::Arc::new(vec![Val::I(3), Val::I(5)])));
}

#[test]
fn fold_other_mask_declined() {
    let (m, reps) = analyzed(
        "def m255(n):\n    s = 0\n    for i in range(n):\n        s = (s + i) & 255\n    return s\n",
    );
    let r = report(&reps, "m255");
    assert!(!r.proven, "must not be proven: {}", r.reason);
    assert!(r.reason.contains("opaque"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "m255"), &None);
}

#[test]
fn fold_unmasked_result_declined() {
    // an inner `& 4294967295` without the combiner's top-level result
    // being masked is unsound to strip: decline.
    let (m, reps) = analyzed(
        "def um(n):\n    s = 0\n    for i in range(n):\n        s = (s & 4294967295) + i\n    return s\n",
    );
    let r = report(&reps, "um");
    assert!(!r.proven, "must not be proven: {}", r.reason);
    assert!(r.reason.contains("masked"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "um"), &None);
}

#[test]
fn fold_partially_masked_tuple_declined() {
    let src = "\
def hmix(a, b):
    return ((a[0] + b[0]) & 4294967295, a[1] + b[1])

def histmix(n):
    h = (0, 0)
    for i in range(n):
        h = hmix(h, (i, i))
    return h
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "histmix");
    assert!(!r.proven, "must not be proven: {}", r.reason);
    assert!(r.reason.contains("component"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "histmix"), &None);
}

#[test]
fn lean_obligations_masked_use_bitvec_32_and_check() {
    let src = "\
def hadd32(a, b):
    return ((a[0] + b[0]) & 4294967295, (a[1] + b[1]) & 4294967295)

def hist32(n):
    h = (0, 0)
    for i in range(n):
        h = hadd32(h, (i, i*i))
    return h

def total32(n):
    s = 0
    for i in range(n):
        s = (s + i) & 4294967295
    return s
";
    let (_m, reps) = analyzed(src);
    let text = lean_obligations(&reps);
    assert!(text.contains("Fin 2 → BitVec 32"), "text:\n{text}");
    assert!(text.contains("(a b : BitVec 32)"), "text:\n{text}");
    assert!(!text.contains("BitVec 56"), "masked-only module must not emit BitVec 56:\n{text}");
    let home = std::env::var("HOME").unwrap_or_default();
    let lean = std::path::Path::new(&home).join(".elan/bin/lean");
    if !lean.exists() {
        eprintln!("skipping lean check: {} not found", lean.display());
        return;
    }
    let dir = std::env::temp_dir().join("mithril-reassoc-test");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let file = dir.join("obligations32.lean");
    std::fs::write(&file, &text).expect("write obligations32.lean");
    let out = std::process::Command::new(&lean).arg(&file).output().expect("failed to run lean");
    assert!(
        out.status.success(),
        "lean rejected the BitVec 32 obligations:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- declined folds ----

#[test]
fn fold_affine_declined_not_associative() {
    let (m, reps) = analyzed(
        "def affine(n, x):\n    m = 0\n    for i in range(n):\n        m = m*3 + x\n    return m\n",
    );
    let r = report(&reps, "affine");
    assert!(!r.proven);
    assert!(r.reason.contains("associat"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "affine"), &None);
}

#[test]
fn fold_subtraction_declined_not_associative() {
    let (m, reps) =
        analyzed("def diff(n):\n    s = 0\n    for i in range(n):\n        s = s - i\n    return s\n");
    let r = report(&reps, "diff");
    assert!(!r.proven);
    assert!(r.reason.contains("associat"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "diff"), &None);
}

#[test]
fn fold_product_declined_no_zero_identity() {
    let (m, reps) =
        analyzed("def prod(n):\n    p = 0\n    for i in range(n):\n        p = p * i\n    return p\n");
    let r = report(&reps, "prod");
    assert!(!r.proven);
    assert!(r.reason.contains("identity"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "prod"), &None);
}

#[test]
fn fold_non_identity_init_declined() {
    let (m, reps) =
        analyzed("def bias(n):\n    s = 5\n    for i in range(n):\n        s = s + i\n    return s\n");
    let r = report(&reps, "bias");
    assert!(!r.proven);
    assert!(r.reason.contains("identity"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "bias"), &None);
}

#[test]
fn fold_init_clobbered_by_enclosing_while_declined() {
    // `s` is 0 only on the *first* entry of the inner loop; the while may
    // re-enter it with the accumulated value, so init != identity.
    let src = "\
def loopy(n):
    s = 0
    t = 0
    while t < n:
        for i in range(n):
            s = s + i
        t = t + 1
    return s
";
    let (_m, reps) = analyzed(src);
    let r = report(&reps, "loopy");
    assert!(!r.proven);
    assert!(r.reason.contains("identity"), "reason: {}", r.reason);
}

#[test]
fn fold_last_element_combiner_declined_unsupported_form() {
    // step is `b` (assoc + identity hold!) but it is not a wrapping add,
    // which is the only combiner shape the pipeline carries.
    let (m, reps) =
        analyzed("def last(n):\n    s = 0\n    for i in range(n):\n        s = s*0 + i\n    return s\n");
    let r = report(&reps, "last");
    assert!(!r.proven);
    assert!(r.reason.contains("wrapping add"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "last"), &None);
}

#[test]
fn fold_call_elem_reading_acc_declined() {
    // form 1 where the element expression reads the accumulator:
    // reassociation would miscompile (element is not per-iteration data).
    let src = "\
def add(a, b):
    return a + b

def bad(n):
    s = 0
    for i in range(n):
        s = add(s, s + i)
    return s
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "bad");
    assert!(!r.proven, "must not be proven: {}", r.reason);
    assert!(r.reason.contains("accumulator"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "bad"), &None);
}

#[test]
fn fold_call_elem_via_helper_reading_acc_declined() {
    let src = "\
def add(a, b):
    return a + b

def h(x):
    return x * 2

def bad2(n):
    s = 0
    for i in range(n):
        s = add(s, h(s))
    return s
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "bad2");
    assert!(!r.proven, "must not be proven: {}", r.reason);
    assert!(r.reason.contains("accumulator"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "bad2"), &None);
}

#[test]
fn fold_opaque_combiner_declined() {
    let src = "\
def half(a, b):
    return (a + b) // 2

def avg(n):
    s = 0
    for i in range(n):
        s = half(s, i)
    return s
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "avg");
    assert!(!r.proven);
    assert!(r.reason.contains("opaque"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "avg"), &None);
}

#[test]
fn fold_combiner_not_single_return_declined() {
    let src = "\
def cadd(a, b):
    c = a + b
    return c

def tot(n):
    s = 0
    for i in range(n):
        s = cadd(s, i)
    return s
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "tot");
    assert!(!r.proven);
    assert!(r.reason.contains("single return"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "tot"), &None);
}

#[test]
fn multi_statement_loop_body_is_not_a_fold() {
    let src = "\
def noacc(n):
    s = 0
    t = 0
    for i in range(n):
        s = s + i
        t = t + i
    return s + t
";
    let (m, reps) = analyzed(src);
    let r = report(&reps, "noacc");
    assert!(!r.proven);
    assert_eq!(r.acc, "");
    assert!(r.reason.contains("not a single-assignment accumulation"), "reason: {}", r.reason);
    assert_eq!(fold_of(&m, "noacc"), &None);
}

// ---- sequential semantics (desugar + eval_core oracle) ----

#[test]
fn fold_empty_range() {
    // A proven fold over range(0) still evaluates, sequentially via
    // eval_core, to the loop's init value.
    let (m, reps) = analyzed(SRC_SUM);
    assert!(report(&reps, "total").proven);
    let cm = desugar(&m).expect("desugar failed");
    let helper = cm.fns.iter().find(|f| f.fold.is_some()).expect("no CoreFn carries fold info");
    assert_eq!(helper.fold.as_ref().unwrap().combiner, CoreCombiner::WrapAdd);
    let total = fn_id(&cm, "total");
    assert_eq!(eval_core(&cm, total, &[Val::I(0)]), Val::I(0));
    // non-empty sanity: f(0)+f(1)+f(2) = 1 + 2 + 5
    assert_eq!(eval_core(&cm, total, &[Val::I(3)]), Val::I(8));
}

// ---- Lean obligations ----

const GOLDEN: &str = "\
-- GENERATED proof obligations for fold reassociation (mithril-reassoc).
-- One obligation pair per transformed fold; the generic
-- foldl == treeReduce lemma is proved once below.

abbrev V8 := Fin 8 → BitVec 56
def comb_hist_h (a b : V8) : V8 := fun i => a i + b i
theorem assoc_hist_h : ∀ a b c, comb_hist_h (comb_hist_h a b) c = comb_hist_h a (comb_hist_h b c) := by
  intro a b c; funext i; simp [comb_hist_h, BitVec.add_assoc]
theorem ident_hist_h : ∀ b, comb_hist_h (fun _ => 0) b = b := by
  intro b; funext i; simp [comb_hist_h]

def comb_recolor_s (a b : BitVec 56) : BitVec 56 := a + b
theorem assoc_recolor_s : ∀ a b c, comb_recolor_s (comb_recolor_s a b) c = comb_recolor_s a (comb_recolor_s b c) := by
  intro a b c; simp [comb_recolor_s, BitVec.add_assoc]
theorem ident_recolor_s : ∀ b, comb_recolor_s 0 b = b := by
  intro b; simp [comb_recolor_s]

-- Generic justification, proved once for the compiler: folding chunk
-- results equals folding the concatenation (leaf order preserved).
theorem chunked_foldl {α} (f : α → α → α) (z : α)
    (xs ys : List α) :
    List.foldl f (List.foldl f z xs) ys = List.foldl f z (xs ++ ys) := by
  simp [List.foldl_append]
";

#[test]
fn lean_obligations_golden() {
    let (_m, reps) = analyzed(SRC_GOLDEN);
    assert!(report(&reps, "hist").proven, "{:?}", reps);
    assert!(report(&reps, "recolor").proven, "{:?}", reps);
    assert_eq!(lean_obligations(&reps), GOLDEN);
}

#[test]
fn lean_obligations_no_proven_folds_is_just_the_generic_lemma() {
    let text = lean_obligations(&[]);
    assert!(text.contains("chunked_foldl"));
    assert!(!text.contains("comb_"));
}

#[test]
#[should_panic(expected = "has no arity")]
fn lean_obligations_rejects_proven_report_without_arity() {
    // A proven report must carry its combiner arity; a malformed one must
    // fail loudly rather than silently emit a wrong-shaped obligation.
    let r = FoldReport {
        func: "f".into(),
        acc: "s".into(),
        proven: true,
        reason: "PROVEN".into(),
        arity: 0,
        bits: 0,
    };
    lean_obligations(&[r]);
}

#[test]
fn lean_obligations_check_with_lean() {
    let (_m, reps) = analyzed(SRC_GOLDEN);
    let text = lean_obligations(&reps);
    let home = std::env::var("HOME").unwrap_or_default();
    let lean = std::path::Path::new(&home).join(".elan/bin/lean");
    if !lean.exists() {
        eprintln!("skipping lean check: {} not found", lean.display());
        return;
    }
    let dir = std::env::temp_dir().join("mithril-reassoc-test");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let file = dir.join("obligations.lean");
    std::fs::write(&file, &text).expect("write obligations.lean");
    let out = std::process::Command::new(&lean).arg(&file).output().expect("failed to run lean");
    assert!(
        out.status.success(),
        "lean rejected the obligations:\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- per-iteration locals and addition chains ----

const LOCALS_CHAIN: &str = "def g(x):\n    return x * 3 + 1\n\ndef f(n):\n    s = 0\n    for i in range(n):\n        a = g(i)\n        b = a * a\n        s = (s + g(a) + b + i) & 4294967295\n    return s\n";

#[test]
fn fold_with_local_prefix_and_add_chain_proven() {
    let (m, reports) = analyzed(LOCALS_CHAIN);
    assert!(report(&reports, "f").proven, "{:?}", report(&reports, "f"));
    assert!(matches!(fold_of(&m, "f"), Some(FoldInfo { combiner: Combiner::WrapAdd32, proven: true })));
    // sequential semantics unchanged
    let cm = desugar(&m).unwrap();
    let f = fn_id(&cm, "f");
    let want: i64 = (0..50i64).fold(0, |s, i| {
        let a = i * 3 + 1;
        (s + (a * 3 + 1) + a * a + i) & 4294967295
    });
    assert_eq!(eval_core(&cm, f, &[Val::I(50)]), Val::I(want));
}

#[test]
fn fold_prefix_reading_the_accumulator_declined() {
    let src = "def f(n):\n    s = 0\n    for i in range(n):\n        a = s + i\n        s = s + a\n    return s\n";
    let (m, reports) = analyzed(src);
    assert!(!report(&reports, "f").proven);
    assert!(fold_of(&m, "f").is_none());
}

#[test]
fn fold_loop_carried_local_declined() {
    // `t` is read before it is assigned in the body: it carries a value
    // from the previous iteration, so iterations are not independent
    let src = "def f(n):\n    s = 0\n    t = 1\n    for i in range(n):\n        u = t * 2\n        t = i\n        s = s + u\n    return s\n";
    let (m, reports) = analyzed(src);
    assert!(!report(&reports, "f").proven);
    assert!(fold_of(&m, "f").is_none());
}

#[test]
fn fold_chain_with_two_accumulator_leaves_declined() {
    // s + s + i is 2s + i: not an accumulation of per-iteration data
    let src = "def f(n):\n    s = 0\n    for i in range(n):\n        s = s + s + i\n    return s\n";
    let (m, reports) = analyzed(src);
    assert!(!report(&reports, "f").proven);
    assert!(fold_of(&m, "f").is_none());
}
