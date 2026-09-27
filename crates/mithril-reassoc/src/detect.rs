//! Fold detection over the surface AST plus the associativity/identity
//! prover: a Rust port of spike 4's detector, generalized from Z_2^32 to
//! Z_2^56. Anything outside the provable fragment is *declined* (the loop
//! stays sequential) — soundness over coverage.

use crate::poly::{padd, pconst, pmul, pneg, psubst, pvar, Poly};
use mithril_front::ast::{BinOp, Expr, FnDef, Stmt};
use std::collections::HashMap;

/// A detected accumulation loop `for v in range(n): acc = <combiner>`.
pub enum Cand<'a> {
    /// Form 1: `acc = f(acc, elem)` — `f` inlined symbolically.
    Call { acc: &'a str, fname: &'a str },
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

/// Match the body of `for var in range(_):` against the two supported
/// accumulation shapes. `None` = not an accumulation loop at all.
pub fn detect<'a>(var: &str, body: &'a [Stmt]) -> Option<Cand<'a>> {
    let (acc, val) = match body {
        [Stmt::Assign(acc, val)] => (acc.as_str(), val),
        _ => return None,
    };
    if acc == var {
        return None; // "accumulating" into the induction variable
    }
    if let Expr::Call(fname, args) = val {
        if let [Expr::Var(a0), _] = args.as_slice() {
            if a0 == acc {
                return Some(Cand::Call { acc, fname });
            }
        }
        return None;
    }
    if let Expr::Bin(op, left, right) = val {
        if uses_var(left, acc) && !uses_var(right, acc) {
            return Some(Cand::Expr { acc, op: *op, left });
        }
    }
    None
}

fn uses_var(e: &Expr, name: &str) -> bool {
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

/// Symbolically evaluate an i56 expression to a `Poly`. `Err` = opaque:
/// outside the wrapping `+ - *` fragment (div, shifts, comparisons,
/// non-full bitand, calls, ...), with a human-readable reason.
pub fn sym_eval(e: &Expr, env: &HashMap<String, Poly>) -> Result<Poly, String> {
    match e {
        Expr::Int(n) => Ok(pconst(*n)),
        Expr::Var(n) => env.get(n).cloned().ok_or_else(|| format!("free variable '{n}'")),
        Expr::Index(base, idx) => {
            if let (Expr::Var(n), Expr::Int(j)) = (base.as_ref(), idx.as_ref()) {
                if let Some(p) = env.get(&format!("{n}[{j}]")) {
                    return Ok(p.clone());
                }
            }
            Err("tuple indexing outside the combiner parameters".into())
        }
        Expr::Bin(op, a, b) => match op {
            BinOp::Add => Ok(padd(&sym_eval(a, env)?, &sym_eval(b, env)?)),
            BinOp::Sub => Ok(padd(&sym_eval(a, env)?, &pneg(&sym_eval(b, env)?))),
            BinOp::Mul => Ok(pmul(&sym_eval(a, env)?, &sym_eval(b, env)?)),
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
/// polynomial over `a0..a{arity-1}`, `b0..b{arity-1}`.
pub struct Step {
    pub polys: Vec<Poly>,
    pub arity: usize,
}

/// Build the step of `acc = f(acc, elem)` by symbolically inlining `f`,
/// which must be a single `return`; a tuple return is componentwise, with
/// the tuple parameters accessed by constant subscript (`a[i]`, `b[i]`).
pub fn step_from_fn(f: &FnDef) -> Result<Step, String> {
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
            let mut env = HashMap::new();
            for j in 0..k {
                env.insert(format!("{pa}[{j}]"), pvar(&format!("a{j}")));
                env.insert(format!("{pb}[{j}]"), pvar(&format!("b{j}")));
            }
            let polys = elts.iter().map(|e| sym_eval(e, &env)).collect::<Result<Vec<_>, _>>()?;
            Ok(Step { polys, arity: k })
        }
        e => {
            let mut env = HashMap::new();
            env.insert(pa.clone(), pvar("a0"));
            env.insert(pb.clone(), pvar("b0"));
            Ok(Step { polys: vec![sym_eval(e, &env)?], arity: 1 })
        }
    }
}

/// Build the step of `acc = left op elem`: `acc -> a0` and the (opaque,
/// acc-free) element subtree `-> b0`.
pub fn step_from_expr(acc: &str, op: BinOp, left: &Expr) -> Result<Step, String> {
    let mut env = HashMap::new();
    env.insert(acc.to_string(), pvar("a0"));
    let l = sym_eval(left, &env)?;
    let b = pvar("b0");
    let p = match op {
        BinOp::Add => padd(&l, &b),
        BinOp::Sub => padd(&l, &pneg(&b)),
        BinOp::Mul => pmul(&l, &b),
        other => return Err(format!("operator {other:?} is outside the wrapping +,-,* fragment")),
    };
    Ok(Step { polys: vec![p], arity: 1 })
}

/// Prove `(associative, zero-is-left-identity)` for a combiner step by
/// polynomial normal-form equality over Z_2^56.
pub fn prove(step: &Step) -> (bool, bool) {
    let k = step.arity;
    let vecp = |pfx: &str| -> Vec<Poly> { (0..k).map(|i| pvar(&format!("{pfx}{i}"))).collect() };
    let apply = |a: &[Poly], b: &[Poly]| -> Vec<Poly> {
        let mut env = HashMap::new();
        for i in 0..k {
            env.insert(format!("a{i}"), a[i].clone());
            env.insert(format!("b{i}"), b[i].clone());
        }
        step.polys.iter().map(|p| psubst(p, &env)).collect()
    };
    let (xs, ys, zs) = (vecp("x"), vecp("y"), vecp("z"));
    let assoc = apply(&apply(&xs, &ys), &zs) == apply(&xs, &apply(&ys, &zs));
    let zeros = vec![Poly::new(); k];
    let ident = apply(&zeros, &ys) == ys;
    (assoc, ident)
}

/// True iff the step is exactly componentwise wrapping add — the only
/// combiner shape the pipeline (Combiner enum, codegen join, Lean
/// emission) carries today.
pub fn is_componentwise_add(step: &Step) -> bool {
    (0..step.arity)
        .all(|i| step.polys[i] == padd(&pvar(&format!("a{i}")), &pvar(&format!("b{i}"))))
}
