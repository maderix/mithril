//! Rule-form emission: ANF normalization plus event-driven compilation of a
//! function body into a CALL-rule expansion and continuation segments.
//!
//! Normal form invariant: every `Call` is either the RHS of a `Let` (with
//! call-free arguments) or a tail call with call-free arguments; an `If` or
//! `Match` that contains calls only appears as a `Let` RHS or in tail
//! position, with a call-free condition/scrutinee.
//!
//! Emission: call-free code runs inline inside a fire; each let-bound call
//! becomes a waiting record + a spawned CALL redex, and the let body becomes
//! a segment rule that fires when the record fills. Two adjacent independent
//! calls share one pend-2 record (the fork shape of the bitonic spike). A
//! segment's extra environment is spilled into a `[value, next]` cell chain
//! whose head rides in the record's `d` word.

use crate::seq::Ex;
use crate::{free_vars, has_call};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::BTreeSet;

#[derive(Clone)]
pub(crate) struct Seg {
    pub id: u16,
    pub slots: Vec<u32>,
    pub env: Vec<u32>,
    pub body: Core,
}

pub(crate) struct SegQ {
    pub next: u16,
    pub q: Vec<Seg>,
}

impl SegQ {
    fn add(&mut self, slots: Vec<u32>, env: Vec<u32>, body: Core) -> u16 {
        let id = self.next;
        self.next += 1;
        self.q.push(Seg { id, slots, env, body });
        id
    }
}

// ---- ANF normalization ----

fn fresh(c: &mut u32) -> u32 {
    let v = *c;
    *c += 1;
    v
}

fn wrap(lets: Vec<(u32, Core)>, tail: Core) -> Core {
    lets.into_iter().rev().fold(tail, |acc, (x, r)| Core::Let(x, Box::new(r), Box::new(acc)))
}

/// Normalize a function body (tail position).
pub(crate) fn normalize(e: &Core, c: &mut u32) -> Core {
    norm_tail(e, c)
}

fn norm_tail(e: &Core, c: &mut u32) -> Core {
    if !has_call(e) {
        return e.clone();
    }
    match e {
        Core::Call(f, args) => {
            let mut lets = Vec::new();
            let args2: Vec<Core> = args.iter().map(|a| norm_pure(a, c, &mut lets)).collect();
            wrap(lets, Core::Call(*f, args2))
        }
        Core::If(cd, t, f) => {
            let mut lets = Vec::new();
            let c2 = norm_pure(cd, c, &mut lets);
            wrap(
                lets,
                Core::If(Box::new(c2), Box::new(norm_tail(t, c)), Box::new(norm_tail(f, c))),
            )
        }
        Core::Match(s, arms) => {
            let mut lets = Vec::new();
            let s2 = norm_pure(s, c, &mut lets);
            let arms2 =
                arms.iter().map(|(k, bs, b)| (*k, bs.clone(), norm_tail(b, c))).collect();
            wrap(lets, Core::Match(Box::new(s2), arms2))
        }
        Core::Let(x, r, bo) => {
            let mut lets = Vec::new();
            let r2 = norm_bind(r, c, &mut lets);
            wrap(lets, Core::Let(*x, Box::new(r2), Box::new(norm_tail(bo, c))))
        }
        _ => {
            let mut lets = Vec::new();
            let p = norm_pure(e, c, &mut lets);
            wrap(lets, p)
        }
    }
}

/// Normalize into a let-RHS shape: pure | Call | If | Match; prerequisite
/// bindings are pushed onto `lets`.
fn norm_bind(e: &Core, c: &mut u32, lets: &mut Vec<(u32, Core)>) -> Core {
    if !has_call(e) {
        return e.clone();
    }
    match e {
        Core::Call(f, args) => {
            Core::Call(*f, args.iter().map(|a| norm_pure(a, c, lets)).collect())
        }
        Core::If(cd, t, f) => {
            let c2 = norm_pure(cd, c, lets);
            Core::If(Box::new(c2), Box::new(norm_tail(t, c)), Box::new(norm_tail(f, c)))
        }
        Core::Match(s, arms) => {
            let s2 = norm_pure(s, c, lets);
            Core::Match(
                Box::new(s2),
                arms.iter().map(|(k, bs, b)| (*k, bs.clone(), norm_tail(b, c))).collect(),
            )
        }
        Core::Let(x, r, bo) => {
            let r2 = norm_bind(r, c, lets);
            lets.push((*x, r2));
            norm_bind(bo, c, lets)
        }
        _ => norm_pure(e, c, lets),
    }
}

/// Normalize into a call-free (pure) expression, hoisting any embedded
/// calls / call-carrying Ifs and Matches into `lets`.
fn norm_pure(e: &Core, c: &mut u32, lets: &mut Vec<(u32, Core)>) -> Core {
    if !has_call(e) {
        return e.clone();
    }
    match e {
        Core::Call(..) | Core::If(..) | Core::Match(..) => {
            let r = norm_bind(e, c, lets);
            let x = fresh(c);
            lets.push((x, r));
            Core::Var(x)
        }
        Core::Let(x, r, bo) => {
            let r2 = norm_bind(r, c, lets);
            lets.push((*x, r2));
            norm_pure(bo, c, lets)
        }
        Core::Op2(op, a, b) => Core::Op2(
            *op,
            Box::new(norm_pure(a, c, lets)),
            Box::new(norm_pure(b, c, lets)),
        ),
        Core::Cmp(op, a, b) => Core::Cmp(
            *op,
            Box::new(norm_pure(a, c, lets)),
            Box::new(norm_pure(b, c, lets)),
        ),
        Core::Ctor(k, args) => {
            Core::Ctor(*k, args.iter().map(|a| norm_pure(a, c, lets)).collect())
        }
        Core::Tuple(items) => {
            Core::Tuple(items.iter().map(|a| norm_pure(a, c, lets)).collect())
        }
        Core::Proj(a, i) => Core::Proj(Box::new(norm_pure(a, c, lets)), *i),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
    }
}

// ---- rule-form emission ----

/// Allocate a waiting record for segment `rule` with the given spilled
/// environment; returns the record temp name.
fn emit_rec(ex: &mut Ex, env: &[u32], rule: u16, pend: u32, par: &str, b: &mut String) -> String {
    let chn = ex.fresh();
    if env.is_empty() {
        b.push_str(&format!("let {chn}: u64 = 0;\n"));
    } else {
        b.push_str(&format!("let mut {chn}: u64 = 0;\n"));
        for v in env.iter().rev() {
            b.push_str(&format!("{chn} = ctx.alloc(v{v}, {chn}) as u64 + 1;\n"));
        }
    }
    let rn = ex.fresh();
    b.push_str(&format!(
        "let {rn} = ctx.alloc_rec({rule}u16, {pend}, {chn} as u32, 0, {par});\n"
    ));
    rn
}

fn emit_spawn(ex: &mut Ex, g: u32, args: &[Core], par: &str, b: &mut String) {
    let es: Vec<String> = args.iter().map(|a| ex.val(a, b)).collect();
    b.push_str(&format!("spawn_call(ctx, {}u16, &[{}], {});\n", 1 + g, es.join(", "), par));
}

/// Emit `e` (normalized) in rule-form tail position, delivering to `par`.
fn rtail(ex: &mut Ex, e: &Core, par: &str, b: &mut String, sq: &mut SegQ) {
    match e {
        Core::Let(x, r, bo) => {
            if !has_call(r) {
                let er = ex.val(r, b);
                b.push_str(&format!("let v{x} = {er};\n"));
                return rtail(ex, bo, par, b, sq);
            }
            match r.as_ref() {
                Core::Call(g, gargs) => {
                    // Pair fork: two adjacent independent calls share a record.
                    if let Core::Let(y, r2, bo2) = bo.as_ref() {
                        if let Core::Call(h, hargs) = r2.as_ref() {
                            let indep = !hargs.iter().any(|a| free_vars(a).contains(x));
                            if indep {
                                let mut env: BTreeSet<u32> = free_vars(bo2);
                                env.remove(x);
                                env.remove(y);
                                let env: Vec<u32> = env.into_iter().collect();
                                let sid = sq.add(vec![*x, *y], env.clone(), (**bo2).clone());
                                let rn = emit_rec(ex, &env, sid, 2, par, b);
                                emit_spawn(ex, *g, gargs, &format!("(({rn} as u64) << 3)"), b);
                                emit_spawn(
                                    ex,
                                    *h,
                                    hargs,
                                    &format!("((({rn} as u64) << 3) | 1)"),
                                    b,
                                );
                                return;
                            }
                        }
                    }
                    let mut env = free_vars(bo);
                    env.remove(x);
                    let env: Vec<u32> = env.into_iter().collect();
                    let sid = sq.add(vec![*x], env.clone(), (**bo).clone());
                    let rn = emit_rec(ex, &env, sid, 1, par, b);
                    emit_spawn(ex, *g, gargs, &format!("(({rn} as u64) << 3)"), b);
                }
                Core::If(..) | Core::Match(..) => {
                    let mut env = free_vars(bo);
                    env.remove(x);
                    let env: Vec<u32> = env.into_iter().collect();
                    let sid = sq.add(vec![*x], env.clone(), (**bo).clone());
                    let rn = emit_rec(ex, &env, sid, 1, par, b);
                    let p2 = format!("(({rn} as u64) << 3)");
                    rtail(ex, r, &p2, b, sq);
                }
                _ => unreachable!("codegen bug: non-normalized let RHS carrying a call"),
            }
        }
        Core::If(c, t, f) => {
            let ec = ex.val(c, b);
            b.push_str(&format!("if as_i({ec}) != 0 {{\n"));
            rtail(ex, t, par, b, sq);
            b.push_str("} else {\n");
            rtail(ex, f, par, b, sq);
            b.push_str("}\n");
        }
        Core::Match(s, arms) => {
            let es = ex.val(s, b);
            b.push_str(&format!("match con_tag({es}) {{\n"));
            for (cid, binders, body) in arms {
                if *cid == UNREACHABLE_CTOR {
                    continue;
                }
                b.push_str(&format!("{cid} => {{\n"));
                for (i, bv) in binders.iter().enumerate() {
                    b.push_str(&format!("let v{bv} = field(ctx, {es}, {i});\n"));
                }
                rtail(ex, body, par, b, sq);
                b.push_str("}\n");
            }
            b.push_str("_ => { mith_unreachable(); }\n}\n");
        }
        Core::Call(g, args) => {
            emit_spawn(ex, *g, args, par, b);
        }
        other => {
            let v = ex.val(other, b);
            b.push_str(&format!("ctx.deliver({par}, {v});\n"));
        }
    }
}

/// The rule-form expansion of function `fid` (used when its dive unwinds):
/// evaluates the body event-driven from the original arguments.
pub(crate) fn expand_fn(m: &CoreModule, fid: u32, body: &Core, sq: &mut SegQ) -> String {
    let ar = m.fns[fid as usize].arity;
    let params: String = (0..ar).map(|i| format!(", v{i}: u64")).collect();
    let mut s = format!("fn x_{fid}(ctx: &mut Wctx, parent: u64{params}) {{\n");
    let mut ex = Ex::new(false, fid, false);
    let mut bb = String::new();
    rtail(&mut ex, body, "parent", &mut bb, sq);
    s.push_str(&bb);
    s.push_str("}\n\n");
    s
}

/// A continuation segment: fires when its record fills; slots arrive in
/// `e.a` / `e.b`, the spilled environment is read back off the cell chain.
pub(crate) fn segment_fn(_m: &CoreModule, seg: &Seg, sq: &mut SegQ) -> String {
    let mut s = format!("fn sg_{}(ctx: &mut Wctx, e: Redex) {{\n", seg.id);
    s.push_str("let inf = ctx.rec(e.aux as u32);\nlet parent = inf.parent;\n");
    s.push_str(&format!("let v{} = e.a;\n", seg.slots[0]));
    if seg.slots.len() > 1 {
        s.push_str(&format!("let v{} = e.b;\n", seg.slots[1]));
    }
    if !seg.env.is_empty() {
        s.push_str("let mut ch: u64 = inf.d as u64;\n");
        for v in &seg.env {
            s.push_str(&format!(
                "let v{v} = {{ let c = ctx.cell((ch - 1) as u32); ch = c[1]; c[0] }};\n"
            ));
        }
    }
    let mut ex = Ex::new(false, u32::MAX, false);
    let mut bb = String::new();
    rtail(&mut ex, &seg.body, "parent", &mut bb, sq);
    s.push_str(&bb);
    s.push_str("}\n\n");
    s
}
