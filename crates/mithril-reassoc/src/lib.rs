//! mithril-reassoc: fold detection + associativity prover + Lean proof
//! obligations (Task 4; Rust port of spike 4 over the surface AST,
//! generalized from Z_2^32 to Z_2^64).
//!
//! `analyze` walks every `for v in range(n)` loop, matches the two
//! accumulation shapes (`acc = f(acc, e)` with `f` inlined symbolically,
//! and the `acc = acc ⊕ e` binop spine), proves the combiner associative
//! with 0 (or the all-zero tuple) as identity by polynomial normal form
//! over Z_2^64, checks the accumulator provably holds that identity when
//! the loop is entered, and marks proven folds on `Stmt::For`'s `fold`
//! field for Task 3's desugar to carry into `CoreFn.fold`. Everything
//! else is declined with a reason — soundness over coverage.

pub mod detect;
pub mod lean;
pub mod poly;
pub mod split;

pub use lean::lean_obligations;

use detect::{detect, is_componentwise_add, prove, step_from_expr, step_from_fn, uses_var, Cand, Step};
use mithril_front::ast::{BinOp, CmpOp, Combiner, Expr, FnDef, FoldInfo, Module, Stmt};
use mithril_front::desugar::assigned_names;
use std::collections::HashMap;

/// One analyzed `for` loop. For loops that are not accumulation-shaped at
/// all, `acc` is empty. `arity` (1 = scalar, k = k-tuple componentwise)
/// and `bits` (64 native, 32 for `& 4294967295`-masked u32-emulation
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
    /// A proven index fill (no combiner: arity and bits are 0).
    pub fill: bool,
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
    for f in &mut m.fns {
        let whole = f.body.clone();
        let mut fresh = 0;
        f.body = collapse_block(std::mem::take(&mut f.body), &whole, &mut fresh);
    }
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
            Stmt::Return(_) | Stmt::ExprStmt(_) | Stmt::Break | Stmt::Continue => {}
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

/// Collapse every row-major nested fill in `stmts` (see `collapse`).
fn collapse_block(stmts: Vec<Stmt>, whole: &[Stmt], fresh: &mut usize) -> Vec<Stmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for st in stmts {
        let st = match st {
            Stmt::If(c, a, b) => Stmt::If(c, collapse_block(a, whole, fresh), collapse_block(b, whole, fresh)),
            Stmt::While(c, b) => Stmt::While(c, collapse_block(b, whole, fresh)),
            Stmt::Match(e, arms) => Stmt::Match(e, arms.into_iter().map(|(p, b)| (p, collapse_block(b, whole, fresh))).collect()),
            Stmt::For(v, n, b, fold) => match collapse(&v, &n, &b, whole, *fresh) {
                Some(flat) => {
                    *fresh += 1;
                    out.extend(flat);
                    continue;
                }
                None => Stmt::For(v, n, collapse_block(b, whole, fresh), fold),
            },
            st => st,
        };
        out.push(st);
    }
    out
}

/// A row-major nested fill as one index fill. `for y in range(n): for x in
/// range(m): ...; a = array_set(a, y * m + x, e)` writes the indices 0 to
/// n * m - 1 in order, exactly as `for i in range(n * m): y = i // m;
/// x = i % m; ...; a = array_set(a, i, e)` does, and the flat loop is an index
/// fill when its body is one (`fill_target`). `m` must be the same for every
/// row, and `y` and `x` unread outside the loops (their values after an empty
/// row differ). `whole` is the function body, `k` numbers the fresh names.
fn collapse(y: &str, n: &Expr, outer: &[Stmt], whole: &[Stmt], k: usize) -> Option<Vec<Stmt>> {
    let [Stmt::For(x, m, inner, None)] = outer else { return None };
    // a `break` or `continue` belongs to the inner loop: merging would change it
    if mithril_front::desugar::has_jump(inner) {
        return None;
    }
    let (Stmt::Assign(a, Expr::Call(f, args)), work) = inner.split_last()? else { return None };
    if f != "array_set" || args.len() != 3 || x == y {
        return None;
    }
    let is = |e: &Expr, v: &str| *e == Expr::Var(v.to_string());
    let row = |e: &Expr| matches!(e, Expr::Bin(BinOp::Mul, p, q) if (is(p, y) && **q == *m) || (**p == *m && is(q, y)));
    if !matches!(&args[1], Expr::Bin(BinOp::Add, p, q) if (row(p) && is(q, x)) || (is(p, x) && row(q))) {
        return None;
    }
    let mut assigned = assigned_names(inner);
    assigned.extend([x.clone(), y.to_string()]);
    if assigned.iter().any(|v| uses_var(m, v)) {
        return None;
    }
    let this = [Stmt::For(y.to_string(), n.clone(), outer.to_vec(), None)];
    if [y, x.as_str()].iter().any(|v| reads_of(whole, v) != reads_of(&this, v)) {
        return None;
    }
    let [rows, cols, cells, i] = ["rows", "cols", "cells", "cell"].map(|p| format!("__{p}{k}"));
    let var = |v: &str| Box::new(Expr::Var(v.to_string()));
    let mut body = vec![
        Stmt::Assign(y.to_string(), Expr::Bin(BinOp::FloorDiv, var(&i), var(&cols))),
        Stmt::Assign(x.clone(), Expr::Bin(BinOp::Mod, var(&i), var(&cols))),
    ];
    body.extend(work.iter().cloned());
    body.push(Stmt::Assign(a.clone(), Expr::Call(f.clone(), vec![args[0].clone(), Expr::Var(i.clone()), args[2].clone()])));
    fill_target(&i, &body)?;
    // no rows: no cells (n * m is positive for two negative bounds)
    let count = Expr::IfExp(
        Box::new(Expr::Cmp(CmpOp::Gt, var(&rows), Box::new(Expr::Int(0)))),
        Box::new(Expr::Bin(BinOp::Mul, var(&rows), var(&cols))),
        Box::new(Expr::Int(0)),
    );
    Some(vec![
        Stmt::Assign(rows, n.clone()),
        Stmt::Assign(cols, m.clone()),
        Stmt::Assign(cells.clone(), count),
        Stmt::For(i, Expr::Var(cells), body, None),
    ])
}

/// How many times `stmts` read the variable `v`.
fn reads_of(stmts: &[Stmt], v: &str) -> usize {
    let at = |e: &Expr| e.fold(&mut |e, kids: Vec<usize>| kids.iter().sum::<usize>() + (*e == Expr::Var(v.to_string())) as usize);
    stmts.iter().map(|st| {
        let (head, blocks) = st.parts();
        at(head) + blocks.iter().map(|b| reads_of(b, v)).sum::<usize>()
    }).sum()
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
    let report = |acc: &str, proven, reason, arity, bits| FoldReport { func: func.to_string(), acc: acc.to_string(), proven, reason, arity, bits, fill: false };
    if let Some(acc) = fill_target(var, body) {
        *fold = Some(FoldInfo { combiner: Combiner::Fill, proven: true });
        let reason = "PROVEN index fill (writes at distinct indices commute)".to_string();
        out.push(FoldReport { fill: true, ..report(acc, true, reason, 0, 0) });
        return;
    }
    out.push(match prove_for(var, body, fns, inits) {
        Ok((acc, combiner, arity, bits, desc)) => {
            *fold = Some(FoldInfo { combiner, proven: true });
            report(acc, true, format!("PROVEN assoc+identity (combiner: {desc}, arity {arity})"), arity, bits)
        }
        Err((acc, reason)) => report(acc, false, reason, 0, 0),
    });
}

/// The array a loop fills, when the loop is an index fill: straight-line
/// assignments to fresh locals, then `a = array_set(a, var, e)`, with `a` read
/// nowhere else in the body and the loop variable never assigned. Each
/// iteration then writes `e` at its own index, and `e` cannot see the writes of
/// other iterations (lemma `fill_chunks`).
fn fill_target<'a>(var: &str, body: &'a [Stmt]) -> Option<&'a str> {
    if mithril_front::desugar::has_jump(body) {
        return None;
    }
    let (Stmt::Assign(a, Expr::Call(f, args)), work) = body.split_last()? else { return None };
    if f != "array_set" || args.len() != 3 || args[0] != Expr::Var(a.clone()) || a == var {
        return None;
    }
    // the index is the loop variable, or a local set to it plus or minus a value
    // the body does not assign (`range(lo, hi)` binds `i = counter + lo`): either
    // way distinct iterations write distinct indices
    let is_var = |e: &Expr| *e == Expr::Var(var.to_string());
    let offset = |e: &Expr| work.iter().all(|st| !matches!(st, Stmt::Assign(x, _) if uses_var(e, x))) && !uses_var(e, a) && !uses_var(e, var);
    let index_ok = match &args[1] {
        e if is_var(e) => true,
        Expr::Var(x) => work.iter().any(|st| match st {
            Stmt::Assign(y, Expr::Bin(BinOp::Add, l, r)) if y == x => (is_var(l) && offset(r)) || (is_var(r) && offset(l)),
            Stmt::Assign(y, Expr::Bin(BinOp::Sub, l, r)) if y == x => is_var(l) && offset(r),
            _ => false,
        }),
        _ => false,
    };
    if !index_ok {
        return None;
    }
    // a local read before its assignment would carry a value between iterations
    let assigned: std::collections::BTreeSet<&String> = work.iter().filter_map(|st| match st {
        Stmt::Assign(x, _) => Some(x),
        _ => None,
    }).collect();
    let mut locals = std::collections::BTreeSet::new();
    let carried = |e: &Expr, locals: &std::collections::BTreeSet<&String>| {
        uses_var(e, a) || assigned.iter().any(|x| !locals.contains(x) && uses_var(e, x))
    };
    for st in work {
        let Stmt::Assign(x, e) = st else { return None };
        if x == a || x == var || locals.contains(x) || carried(e, &locals) {
            return None;
        }
        locals.insert(x);
    }
    if carried(&args[2], &locals) {
        return None;
    }
    Some(a)
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
        return Err(decline("combiner is not associative over Z_2^64, DECLINED".into()));
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
    Ok((acc, combiner, step.arity, if mode32 { 32 } else { 64 }, desc))
}
