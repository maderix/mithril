//! Lean 4 proof-obligation emission: BitVec 56 (native folds) or BitVec
//! 32 (`& 4294967295`-masked u32-emulation folds), same shape as spike
//! 4's `obligations.lean`, including the once-proved generic
//! `chunked_foldl`.

use crate::FoldReport;

/// Emit the Lean obligations for every proven fold in `reports`. A
/// proven fold's obligation shape is fully determined by its `arity` and
/// `bits` (only componentwise wrapping add is ever proven); a proven
/// report with arity 0 or a bit width other than 32/56 is malformed and
/// panics — emitting a wrong-shaped obligation would be far worse than
/// failing loudly.
pub fn lean_obligations(reports: &[FoldReport]) -> String {
    let mut lines: Vec<String> = vec![
        "-- GENERATED proof obligations for fold reassociation (mithril-reassoc).".into(),
        "-- One obligation pair per transformed fold; the generic".into(),
        "-- foldl == treeReduce lemma is proved once below.".into(),
        String::new(),
    ];
    let mut abbrevs: Vec<(usize, u32)> = Vec::new();
    for r in reports.iter().filter(|r| r.proven) {
        let k = r.arity;
        assert!(
            k >= 1,
            "lean_obligations: proven FoldReport for {}::{} has no arity (arity = 0)",
            r.func,
            r.acc
        );
        let bits = r.bits;
        assert!(
            bits == 32 || bits == 56,
            "lean_obligations: proven FoldReport for {}::{} has invalid bit width {} (expected 32 or 56)",
            r.func,
            r.acc,
            bits
        );
        let nm = format!("{}_{}", r.func, r.acc);
        let (ty, sum, fx, zero) = if k == 1 {
            (format!("BitVec {bits}"), "a + b", "", "0")
        } else {
            // Vk is the historical (spike) name for the 56-bit vector;
            // masked folds get a distinct Vkx32 so mixed modules compile.
            let vn = if bits == 56 { format!("V{k}") } else { format!("V{k}x{bits}") };
            if !abbrevs.contains(&(k, bits)) {
                abbrevs.push((k, bits));
                lines.push(format!("abbrev {vn} := Fin {k} → BitVec {bits}"));
            }
            (vn, "fun i => a i + b i", " funext i;", "(fun _ => 0)")
        };
        lines.push(format!("def comb_{nm} (a b : {ty}) : {ty} := {sum}"));
        lines.push(format!(
            "theorem assoc_{nm} : ∀ a b c, comb_{nm} (comb_{nm} a b) c = comb_{nm} a (comb_{nm} b c) := by"
        ));
        lines.push(format!("  intro a b c;{fx} simp [comb_{nm}, BitVec.add_assoc]"));
        lines.push(format!("theorem ident_{nm} : ∀ b, comb_{nm} {zero} b = b := by"));
        lines.push(format!("  intro b;{fx} simp [comb_{nm}]"));
        lines.push(String::new());
    }
    lines.push("-- Generic justification, proved once for the compiler: folding chunk".into());
    lines.push("-- results equals folding the concatenation (leaf order preserved).".into());
    lines.push("theorem chunked_foldl {α} (f : α → α → α) (z : α)".into());
    lines.push("    (xs ys : List α) :".into());
    lines.push("    List.foldl f (List.foldl f z xs) ys = List.foldl f z (xs ++ ys) := by".into());
    lines.push("  simp [List.foldl_append]".into());
    lines.push(String::new());
    lines.join("\n")
}
