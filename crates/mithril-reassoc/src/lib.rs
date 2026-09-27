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

pub use lean::lean_obligations;

use detect::{detect, is_componentwise_add, prove, step_from_expr, step_from_fn, Cand, Step};
use mithril_front::ast::{Combiner, Expr, FnDef, FoldInfo, Module, Stmt};
use std::collections::HashMap;

/// One analyzed `for` loop. For loops that are not accumulation-shaped at
/// all, `acc` is empty. Proven reports' `reason` records the combiner and
/// its arity (`lean_obligations` reads the arity back from it).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FoldReport {
    pub func: String,
    pub acc: String,
    pub proven: bool,
    pub reason: String,
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
    for s in stmts {
        match s {
            Stmt::Assign(n, _) => {
                inits.insert(n.clone(), Init::Other);
            }
            Stmt::If(_, t, e) => {
                kill_assigned(inits, t);
                kill_assigned(inits, e);
            }
            Stmt::While(_, b) => kill_assigned(inits, b),
            Stmt::For(v, _, b, _) => {
                inits.insert(v.clone(), Init::Other);
                kill_assigned(inits, b);
            }
            Stmt::Match(_, cases) => {
                for (_, b) in cases {
                    kill_assigned(inits, b);
                }
            }
            Stmt::Return(_) | Stmt::ExprStmt(_) => {}
        }
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

fn decline(out: &mut Vec<FoldReport>, func: &str, acc: &str, reason: String) {
    out.push(FoldReport { func: func.to_string(), acc: acc.to_string(), proven: false, reason });
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
    let cand = match detect(var, body) {
        None => {
            return decline(
                out,
                func,
                "",
                format!("loop over '{var}' is not a single-assignment accumulation, kept sequential"),
            )
        }
        Some(c) => c,
    };
    let acc = cand.acc();
    let step: Step = match &cand {
        Cand::Call { fname, .. } => match fns.get(*fname).map(step_from_fn) {
            None => {
                return decline(out, func, acc, format!("combiner opaque (unknown function '{fname}'), DECLINED"))
            }
            Some(Err(e)) => return decline(out, func, acc, format!("combiner opaque ({e}), DECLINED")),
            Some(Ok(s)) => s,
        },
        Cand::Expr { op, left, .. } => match step_from_expr(acc, *op, left) {
            Err(e) => return decline(out, func, acc, format!("combiner opaque ({e}), DECLINED")),
            Ok(s) => s,
        },
    };
    let (assoc, ident) = prove(&step);
    if !assoc {
        return decline(out, func, acc, "combiner is not associative over Z_2^56, DECLINED".into());
    }
    if !ident {
        return decline(out, func, acc, "zero is not a left identity of the combiner, DECLINED".into());
    }
    if !is_componentwise_add(&step) {
        return decline(
            out,
            func,
            acc,
            "combiner is associative with identity but not (componentwise) wrapping add — unsupported form, DECLINED".into(),
        );
    }
    let init_ok = match (inits.get(acc), step.arity) {
        (Some(Init::Zero), 1) => true,
        (Some(Init::ZeroTuple(k)), a) => *k == a,
        _ => false,
    };
    if !init_ok {
        let want =
            if step.arity == 1 { "0".to_string() } else { format!("the all-zero {}-tuple", step.arity) };
        return decline(
            out,
            func,
            acc,
            format!("loop entry value of '{acc}' is not the combiner identity (expected {want}), DECLINED"),
        );
    }
    let (combiner, desc) = if step.arity == 1 {
        (Combiner::WrapAdd, "wrapping add".to_string())
    } else {
        (Combiner::TupleWrapAdd(step.arity), format!("componentwise wrapping add on {}-tuple", step.arity))
    };
    *fold = Some(FoldInfo { combiner, proven: true });
    out.push(FoldReport {
        func: func.to_string(),
        acc: acc.to_string(),
        proven: true,
        reason: format!("PROVEN assoc+identity (combiner: {desc}, arity {})", step.arity),
    });
}
