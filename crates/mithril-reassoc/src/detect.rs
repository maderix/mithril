//! Fold detection over the surface AST plus the associativity/identity
//! prover: a Rust port of spike 4's detector, generalized from Z_2^32 to
//! Z_2^56, with a sound masked mode for u32-emulating benchmark ports:
//! combiners whose top-level result is masked with the exact literal
//! `& 4294967295` have all such masks stripped and are proved in Z_2^32
//! instead (the low 32 bits of wrapping `+ - *` depend only on the low
//! 32 bits of the inputs, and the result is re-masked). Anything outside
//! the provable fragment is *declined* (the loop stays sequential) —
//! soundness over coverage.

use crate::poly::{padd, pconst, pmul, pneg, psubst, pvar, Poly, MASK32, MASK56};
use mithril_front::ast::{BinOp, Expr, FnDef, Stmt};
use std::collections::HashMap;

/// The exact u32-emulation mask literal (`4294967295 = 2^32 - 1`).
pub const M32_LIT: i64 = 0xFFFF_FFFF;

/// A detected accumulation loop `for v in range(n): acc = <combiner>`.
pub enum Cand<'a> {
    /// Form 1: `acc = f(acc, elem)` — `f` inlined symbolically.
    Call { acc: &'a str, fname: &'a str, elem: &'a Expr },
    /// Form 2: `acc = <left spine using acc> op <acc-free elem>`.
    Expr { acc: &'a str, op: BinOp, left: &'a Expr },
}

impl<'a> Cand<'a> {
    pub fn acc(&self) -> &'a str {
        match self {
            Cand::Call { acc, .. } | Cand::Expr { acc, .. } => acc,
        }
    }
}

/// If `e` is `<inner> & 4294967295` (either operand order), return the
/// unmasked operand.
pub fn strip_top_mask(e: &Expr) -> Option<&Expr> {
    if let Expr::Bin(BinOp::BitAnd, a, b) = e {
        if matches!(b.as_ref(), Expr::Int(v) if *v == M32_LIT) {
            return Some(a);
        }
        if matches!(a.as_ref(), Expr::Int(v) if *v == M32_LIT) {
            return Some(b);
        }
    }
    None
}

/// Match the body of `for var in range(_):` against the two supported
/// accumulation shapes, after stripping one top-level `& 4294967295`
/// mask if present (the returned bool: the combiner's result is masked,
/// so the proof runs in Z_2^32). `None` = not an accumulation loop.
pub fn detect<'a>(var: &str, body: &'a [Stmt]) -> Option<(Cand<'a>, bool)> {
    let (acc, val0) = match body {
        [Stmt::Assign(acc, val)] => (acc.as_str(), val),
        _ => return None,
    };
    if acc == var {
        return None; // "accumulating" into the induction variable
    }
    let (val, top_masked) = match strip_top_mask(val0) {
        Some(inner) => (inner, true),
        None => (val0, false),
    };
    if let Expr::Call(fname, args) = val {
        if let [Expr::Var(a0), elem] = args.as_slice() {
            if a0 == acc {
                // NOTE: `elem` may still read `acc`; `analyze_for` declines
                // that case explicitly (with a reason) — it must never be
                // proven, since the element would not be per-iteration data.
                return Some((Cand::Call { acc, fname, elem }, top_masked));
            }
        }
        return None;
    }
    if let Expr::Bin(op, left, right) = val {
        if uses_var(left, acc) && !uses_var(right, acc) {
            return Some((Cand::Expr { acc, op: *op, left }, top_masked));
        }
    }
    None
}

pub fn uses_var(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Var(n) => n == name,
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) => false,
        Expr::Bin(_, a, b) | Expr::Cmp(_, a, b) | Expr::Bool2(_, a, b) | Expr::Index(a, b) => {
            uses_var(a, name) || uses_var(b, name)
        }
        Expr::Not(a) => uses_var(a, name),
        Expr::IfExp(c, t, e2) => uses_var(c, name) || uses_var(t, name) || uses_var(e2, name),
        Expr::Call(_, args) | Expr::Tuple(args) => args.iter().any(|a| uses_var(a, name)),
        Expr::Lambda(params, b) => !params.iter().any(|p| p == name) && uses_var(b, name),
    }
}

/// Symbolically evaluate an expression to a `Poly` over Z_2^k (`mask` =
/// 2^k - 1). `Err` = opaque: outside the wrapping `+ - *` fragment (div,
/// shifts, comparisons, calls, ...), with a human-readable reason. In
/// Z_2^32 mode (`mask == MASK32`) `& 4294967295` is the identity and is
/// stripped recursively; in Z_2^56 mode it is opaque, since stripping it
/// is only sound when the combiner's result is re-masked.
pub fn sym_eval(e: &Expr, env: &HashMap<String, Poly>, mask: u64) -> Result<Poly, String> {
    match e {
        Expr::Int(n) => Ok(pconst(*n, mask)),
        Expr::Var(n) => env.get(n).cloned().ok_or_else(|| format!("free variable '{n}'")),
        Expr::Index(base, idx) => {
            if let (Expr::Var(n), Expr::Int(j)) = (base.as_ref(), idx.as_ref()) {
                if let Some(p) = env.get(&format!("{n}[{j}]")) {
                    return Ok(p.clone());
                }
            }
            Err("tuple indexing outside the combiner parameters".into())
        }
        Expr::Bin(BinOp::BitAnd, a, b) => {
            let is_m32 = |x: &Expr| matches!(x, Expr::Int(v) if *v == M32_LIT);
            if mask == MASK32 && is_m32(b.as_ref()) {
                return sym_eval(a, env, mask);
            }
            if mask == MASK32 && is_m32(a.as_ref()) {
                return sym_eval(b, env, mask);
            }
            if is_m32(a.as_ref()) || is_m32(b.as_ref()) {
                Err("subexpression is masked with & 4294967295 but the combiner's top-level result is not masked".into())
            } else {
                Err("bitand with a mask other than 4294967295 is opaque".into())
            }
        }
        Expr::Bin(op, a, b) => match op {
            BinOp::Add => Ok(padd(&sym_eval(a, env, mask)?, &sym_eval(b, env, mask)?, mask)),
            BinOp::Sub => {
                Ok(padd(&sym_eval(a, env, mask)?, &pneg(&sym_eval(b, env, mask)?, mask), mask))
            }
            BinOp::Mul => Ok(pmul(&sym_eval(a, env, mask)?, &sym_eval(b, env, mask)?, mask)),
            other => Err(format!("operator {other:?} is outside the wrapping +,-,* fragment")),
        },
        Expr::Tuple(_) => Err("tuple in scalar position".into()),
        Expr::Call(f, _) => Err(format!("call to '{f}' inside the combiner")),
        Expr::Cmp(..) => Err("comparison inside the combiner".into()),
        Expr::Bool2(..) | Expr::Not(_) | Expr::Bool(_) => Err("boolean inside the combiner".into()),
        Expr::IfExp(..) => Err("conditional inside the combiner".into()),
        Expr::Float(_) => Err("float inside the combiner".into()),
        Expr::Lambda(..) => Err("lambda inside the combiner".into()),
    }
}

/// A combiner in normal form: `polys[i]` is output component `i` as a
/// polynomial over `a0..a{arity-1}`, `b0..b{arity-1}`, with coefficients
/// in Z_2^k (`mask` = MASK56 native, MASK32 for masked u32-emulation).
pub struct Step {
    pub polys: Vec<Poly>,
    pub arity: usize,
    pub mask: u64,
}

/// Build the step of `acc = f(acc, elem)` by symbolically inlining `f`,
/// which must be a single `return`; a tuple return is componentwise, with
/// the tuple parameters accessed by constant subscript (`a[i]`, `b[i]`).
/// `loop_masked`: the loop-level assignment already re-masks the result.
/// Otherwise a scalar return that is top-level `& 4294967295`-masked, or
/// a tuple return with *every* component top-level masked, selects Z_2^32
/// mode; a partially masked tuple is unsound either way and is declined.
pub fn step_from_fn(f: &FnDef, loop_masked: bool) -> Result<Step, String> {
    if f.params.len() != 2 {
        return Err(format!("combiner '{}' must take exactly two parameters", f.name));
    }
    let ret = match f.body.as_slice() {
        [Stmt::Return(e)] => e,
        _ => return Err(format!("combiner '{}' is not a single return", f.name)),
    };
    let (pa, pb) = (&f.params[0], &f.params[1]);
    match ret {
        Expr::Tuple(elts) if !elts.is_empty() => {
            let k = elts.len();
            let n_masked = elts.iter().filter(|e| strip_top_mask(e).is_some()).count();
            let mode32 = loop_masked || n_masked == k;
            if !mode32 && n_masked > 0 {
                return Err(format!(
                    "combiner '{}' masks only {n_masked} of {k} tuple components with & 4294967295 (every component must be masked)",
                    f.name
                ));
            }
            let mask = if mode32 { MASK32 } else { MASK56 };
            let mut env = HashMap::new();
            for j in 0..k {
                env.insert(format!("{pa}[{j}]"), pvar(&format!("a{j}")));
                env.insert(format!("{pb}[{j}]"), pvar(&format!("b{j}")));
            }
            let polys = elts.iter().map(|e| sym_eval(e, &env, mask)).collect::<Result<Vec<_>, _>>()?;
            Ok(Step { polys, arity: k, mask })
        }
        e => {
            let mode32 = loop_masked || strip_top_mask(e).is_some();
            let mask = if mode32 { MASK32 } else { MASK56 };
            let mut env = HashMap::new();
            env.insert(pa.clone(), pvar("a0"));
            env.insert(pb.clone(), pvar("b0"));
            Ok(Step { polys: vec![sym_eval(e, &env, mask)?], arity: 1, mask })
        }
    }
}

/// Build the step of `acc = left op elem`: `acc -> a0` and the (opaque,
/// acc-free) element subtree `-> b0`. `top_masked`: the whole combiner
/// result was `& 4294967295`-masked (already stripped by `detect`), so
/// the proof runs in Z_2^32.
pub fn step_from_expr(acc: &str, op: BinOp, left: &Expr, top_masked: bool) -> Result<Step, String> {
    let mask = if top_masked { MASK32 } else { MASK56 };
    let mut env = HashMap::new();
    env.insert(acc.to_string(), pvar("a0"));
    let l = sym_eval(left, &env, mask)?;
    let b = pvar("b0");
    let p = match op {
        BinOp::Add => padd(&l, &b, mask),
        BinOp::Sub => padd(&l, &pneg(&b, mask), mask),
        BinOp::Mul => pmul(&l, &b, mask),
        other => return Err(format!("operator {other:?} is outside the wrapping +,-,* fragment")),
    };
    Ok(Step { polys: vec![p], arity: 1, mask })
}

/// Prove `(associative, zero-is-left-identity)` for a combiner step by
/// polynomial normal-form equality in the step's ring (Z_2^56 or Z_2^32).
pub fn prove(step: &Step) -> (bool, bool) {
    let k = step.arity;
    let m = step.mask;
    let vecp = |pfx: &str| -> Vec<Poly> { (0..k).map(|i| pvar(&format!("{pfx}{i}"))).collect() };
    let apply = |a: &[Poly], b: &[Poly]| -> Vec<Poly> {
        let mut env = HashMap::new();
        for i in 0..k {
            env.insert(format!("a{i}"), a[i].clone());
            env.insert(format!("b{i}"), b[i].clone());
        }
        step.polys.iter().map(|p| psubst(p, &env, m)).collect()
    };
    let (xs, ys, zs) = (vecp("x"), vecp("y"), vecp("z"));
    let assoc = apply(&apply(&xs, &ys), &zs) == apply(&xs, &apply(&ys, &zs));
    let zeros = vec![Poly::new(); k];
    let ident = apply(&zeros, &ys) == ys;
    (assoc, ident)
}

/// True iff the step is exactly componentwise wrapping add — the only
/// combiner shape the pipeline (Combiner enum, codegen join, Lean
/// emission) carries today (in either ring; the polynomial is the same).
pub fn is_componentwise_add(step: &Step) -> bool {
    (0..step.arity)
        .all(|i| step.polys[i] == padd(&pvar(&format!("a{i}")), &pvar(&format!("b{i}")), step.mask))
}
