//! Lean 4 proof-obligation emission: BitVec 56, same shape as spike 4's
//! `obligations.lean`, including the once-proved generic `chunked_foldl`.

use crate::FoldReport;

/// Emit the Lean obligations for every proven fold in `reports`. A
/// proven fold's obligation shape is fully determined by its `arity`
/// (only componentwise wrapping add is ever proven); a proven report
/// without one (arity 0) is malformed and panics — emitting a
/// wrong-shaped obligation would be far worse than failing loudly.
pub fn lean_obligations(reports: &[FoldReport]) -> String {
    let mut lines: Vec<String> = vec![
        "-- GENERATED proof obligations for fold reassociation (mithril-reassoc).".into(),
        "-- One obligation pair per transformed fold; the generic".into(),
        "-- foldl == treeReduce lemma is proved once below.".into(),
        String::new(),
    ];
    let mut abbrevs: Vec<usize> = Vec::new();
    for r in reports.iter().filter(|r| r.proven) {
        let k = r.arity;
        assert!(
            k >= 1,
            "lean_obligations: proven FoldReport for {}::{} has no arity (arity = 0)",
            r.func,
            r.acc
        );
        let nm = format!("{}_{}", r.func, r.acc);
        if k == 1 {
            lines.push(format!("def comb_{nm} (a b : BitVec 56) : BitVec 56 := a + b"));
            lines.push(format!(
                "theorem assoc_{nm} : ∀ a b c, comb_{nm} (comb_{nm} a b) c = comb_{nm} a (comb_{nm} b c) := by"
            ));
            lines.push(format!("  intro a b c; simp [comb_{nm}, BitVec.add_assoc]"));
            lines.push(format!("theorem ident_{nm} : ∀ b, comb_{nm} 0 b = b := by"));
            lines.push(format!("  intro b; simp [comb_{nm}]"));
        } else {
            if !abbrevs.contains(&k) {
                abbrevs.push(k);
                lines.push(format!("abbrev V{k} := Fin {k} → BitVec 56"));
            }
            lines.push(format!("def comb_{nm} (a b : V{k}) : V{k} := fun i => a i + b i"));
            lines.push(format!(
                "theorem assoc_{nm} : ∀ a b c, comb_{nm} (comb_{nm} a b) c = comb_{nm} a (comb_{nm} b c) := by"
            ));
            lines.push(format!("  intro a b c; funext i; simp [comb_{nm}, BitVec.add_assoc]"));
            lines.push(format!("theorem ident_{nm} : ∀ b, comb_{nm} (fun _ => 0) b = b := by"));
            lines.push(format!("  intro b; funext i; simp [comb_{nm}]"));
        }
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
