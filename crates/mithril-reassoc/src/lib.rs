//! mithril-reassoc: fold detection + associativity prover + Lean proof
//! obligations (Task 4; Rust port of spike 4 over the surface AST,
//! generalized from Z_2^32 to Z_2^56).
//!
//! `analyze` walks every `for v in range(n)` loop, matches the two
//! accumulation shapes (`acc = f(acc, e)` with `f` inlined symbolically,
//! and the `acc = acc ⊕ e` binop spine), proves the combiner associative
//! with 0 (or the all-zero tuple) as identity by polynomial normal form
//! over Z_2^56, checks the accumulator provably holds that identity when
//! the loop is entered, and marks proven folds on `Stmt::For`'s `fold`
//! field for Task 3's desugar to carry into `CoreFn.fold`. Everything
//! else is declined with a reason — soundness over coverage.

pub mod detect;
pub mod lean;
pub mod poly;
pub mod split;

pub use lean::lean_obligations;

use detect::{detect, is_componentwise_add, prove, step_from_expr, step_from_fn, uses_var, Cand, Step};
use mithril_front::ast::{Combiner, Expr, FnDef, FoldInfo, Module, Stmt};
use std::collections::HashMap;

/// One analyzed `for` loop. For loops that are not accumulation-shaped at
/// all, `acc` is empty. `arity` (1 = scalar, k = k-tuple componentwise)
/// and `bits` (56 native, 32 for `& 4294967295`-masked u32-emulation
/// folds) describe the proven combiner and are what `lean_obligations`
/// shapes the obligation from; both are 0 on declined/non-fold reports.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FoldReport {
    pub func: String,
    pub acc: String,
    pub proven: bool,
    pub reason: String,
    pub arity: usize,
    pub bits: u32,
}

/// What the accumulator provably holds when a loop is entered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Init {
    Zero,
    ZeroTuple(usize),
    Other,
}

/// Analyze every function's `for` loops; mark proven folds in place.
pub fn analyze(m: &mut Module) -> Vec<FoldReport> {
    let fns: HashMap<String, FnDef> = m.fns.iter().map(|f| (f.name.clone(), f.clone())).collect();
    let mut out = Vec::new();
    for f in &mut m.fns {
        let name = f.name.clone();
        let mut inits: HashMap<String, Init> =
            f.params.iter().map(|p| (p.clone(), Init::Other)).collect();
        walk_block(&mut f.body, &name, &fns, &mut inits, &mut out);
    }
    split::split_independent_loops(m);
    out
}

fn classify_init(e: &Expr) -> Init {
    match e {
        Expr::Int(0) => Init::Zero,
        Expr::Tuple(items)
            if !items.is_empty() && items.iter().all(|it| matches!(it, Expr::Int(0))) =>
        {
            Init::ZeroTuple(items.len())
        }
        _ => Init::Other,
    }
}

/// Forget everything about names assigned anywhere in `stmts` (a `for`
/// loop's induction variable counts as assigned by the loop).
fn kill_assigned(inits: &mut HashMap<String, Init>, stmts: &[Stmt]) {
    for n in mithril_front::desugar::assigned_names(stmts) {
        inits.insert(n, Init::Other);
    }
}

/// Walk a statement list in execution order, tracking literal-zero inits.
/// Nested blocks get a clone of the current knowledge; blocks that may
/// re-run (`while`/`for` bodies) additionally forget anything they assign
/// themselves, since a later iteration re-enters with mutated state.
fn walk_block(
    stmts: &mut [Stmt],
    func: &str,
    fns: &HashMap<String, FnDef>,
    inits: &mut HashMap<String, Init>,
    out: &mut Vec<FoldReport>,
) {
    for s in stmts.iter_mut() {
        match s {
            Stmt::Assign(n, e) => {
                let v = classify_init(e);
                inits.insert(n.clone(), v);
            }
            Stmt::Return(_) | Stmt::ExprStmt(_) => {}
            Stmt::If(_, t, e) => {
                let mut it = inits.clone();
                walk_block(t, func, fns, &mut it, out);
                let mut ie = inits.clone();
                walk_block(e, func, fns, &mut ie, out);
                kill_assigned(inits, t);
                kill_assigned(inits, e);
            }
            Stmt::While(_, b) => {
                let mut ib = inits.clone();
                kill_assigned(&mut ib, b);
                walk_block(b, func, fns, &mut ib, out);
                kill_assigned(inits, b);
            }
            Stmt::Match(_, cases) => {
                for (p, b) in cases.iter_mut() {
                    let mut ic = inits.clone();
                    for bind in &p.binds {
                        ic.insert(bind.clone(), Init::Other);
                    }
                    walk_block(b, func, fns, &mut ic, out);
                }
                for (_, b) in &*cases {
                    kill_assigned(inits, b);
                }
            }
            Stmt::For(var, _, body, fold) => {
                analyze_for(func, var, body, fold, fns, inits, out);
                let mut ib = inits.clone();
                ib.insert(var.clone(), Init::Other);
                kill_assigned(&mut ib, body);
                walk_block(body, func, fns, &mut ib, out);
                kill_assigned(inits, body);
                inits.insert(var.clone(), Init::Other);
            }
        }
    }
}

/// Detect + prove one `for var in range(_): body` loop; set `fold` iff
/// proven, and always push a report.
fn analyze_for(
    func: &str,
    var: &str,
    body: &[Stmt],
    fold: &mut Option<FoldInfo>,
    fns: &HashMap<String, FnDef>,
    inits: &HashMap<String, Init>,
    out: &mut Vec<FoldReport>,
) {
    let report = |acc: &str, proven, reason, arity, bits| FoldReport { func: func.to_string(), acc: acc.to_string(), proven, reason, arity, bits };
    out.push(match prove_for(var, body, fns, inits) {
        Ok((acc, combiner, arity, bits, desc)) => {
            *fold = Some(FoldInfo { combiner, proven: true });
            report(acc, true, format!("PROVEN assoc+identity (combiner: {desc}, arity {arity})"), arity, bits)
        }
        Err((acc, reason)) => report(acc, false, reason, 0, 0),
    });
}

/// A proven loop: (accumulator, combiner, arity, bits, description); or
/// why it is declined, with the accumulator ("" when not a fold shape).
#[allow(clippy::type_complexity)]
fn prove_for<'a>(
    var: &str,
    body: &'a [Stmt],
    fns: &HashMap<String, FnDef>,
    inits: &HashMap<String, Init>,
) -> Result<(&'a str, Combiner, usize, u32, String), (&'a str, String)> {
    let Some((cand, top_masked)) = detect(var, body) else {
        return Err(("", format!("loop over '{var}' is not a single-assignment accumulation, kept sequential")));
    };
    let acc = cand.acc();
    let decline = |reason: String| (acc, reason);
    let opaque = |e: String| decline(format!("combiner opaque ({e}), DECLINED"));
    let step: Step = match &cand {
        Cand::Call { fname, elem, .. } => {
            if uses_var(elem, acc) {
                // The "element" is not per-iteration data: reassociating
                // would change semantics even for an associative combiner.
                return Err(decline(format!("element expression reads the accumulator '{acc}', DECLINED")));
            }
            let fd = fns.get(*fname).ok_or_else(|| decline(format!("combiner opaque (unknown function '{fname}'), DECLINED")))?;
            step_from_fn(fd, top_masked).map_err(opaque)?
        }
        Cand::Expr { op, left, .. } => step_from_expr(acc, *op, left, top_masked).map_err(opaque)?,
    };
    let (assoc, ident) = prove(&step);
    if !assoc {
        return Err(decline("combiner is not associative over Z_2^56, DECLINED".into()));
    }
    if !ident {
        return Err(decline("zero is not a left identity of the combiner, DECLINED".into()));
    }
    if !is_componentwise_add(&step) {
        return Err(decline(
            "combiner is associative with identity but not (componentwise) wrapping add — unsupported form, DECLINED".into(),
        ));
    }
    let init_ok = match (inits.get(acc), step.arity) {
        (Some(Init::Zero), 1) => true,
        (Some(Init::ZeroTuple(k)), a) => *k == a,
        _ => false,
    };
    if !init_ok {
        let want =
            if step.arity == 1 { "0".to_string() } else { format!("the all-zero {}-tuple", step.arity) };
        return Err(decline(format!("loop entry value of '{acc}' is not the combiner identity (expected {want}), DECLINED")));
    }
    let mode32 = step.mask == poly::MASK32;
    let (combiner, desc) = match (step.arity, mode32) {
        (1, false) => (Combiner::WrapAdd, "wrapping add".to_string()),
        (1, true) => (Combiner::WrapAdd32, "wrapping add mod 2^32".to_string()),
        (k, false) => {
            (Combiner::TupleWrapAdd(k), format!("componentwise wrapping add on {k}-tuple"))
        }
        (k, true) => (
            Combiner::TupleWrapAdd32(k),
            format!("componentwise wrapping add mod 2^32 on {k}-tuple"),
        ),
    };
    Ok((acc, combiner, step.arity, if mode32 { 32 } else { 56 }, desc))
}
