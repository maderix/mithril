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
//! whose head rides in the record's `d` word (the segment frees the chain).
//!
//! Cell discipline (mirrors the dive form, see seq.rs): a fire owns its
//! inputs; matches on last uses free the constructor spine, shared reads
//! deep-copy, spawned arguments transfer ownership to the callee's CALL
//! rule. Frees are deferred into a local `fr` and flushed when the fire
//! returns (fires are committed, so this is just batching).

use crate::seq::Ex;
use crate::{cnt_expr, free_vars, has_call, merge_max, Cnt};
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::BTreeSet;
use std::collections::HashSet;

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

// ---- fork detection (shared by counting and emission) ----

/// `Let(x, Call g, Let(y, Call h, bo2))` with `h`'s args independent of `x`.
fn fork_parts<'e>(x: &u32, bo: &'e Core) -> Option<(&'e u32, &'e u32, &'e Vec<Core>, &'e Core)> {
    if let Core::Let(y, r2, bo2) = bo {
        if let Core::Call(h, hargs) = r2.as_ref() {
            if !hargs.iter().any(|a| free_vars(a).contains(x)) {
                return Some((y, h, hargs, bo2));
            }
        }
    }
    None
}

// ---- use counting for the rule form (mirrors rtail's structure) ----

pub(crate) fn cnt_rule(e: &Core, m: &mut Cnt) {
    match e {
        Core::Let(x, r, bo) => {
            if !has_call(r) {
                cnt_expr(r, m);
                return cnt_rule(bo, m);
            }
            match r.as_ref() {
                Core::Call(_, gargs) => {
                    if let Some((y, _h, hargs, bo2)) = fork_parts(x, bo) {
                        let mut env = free_vars(bo2);
                        env.remove(x);
                        env.remove(y);
                        for v in env {
                            *m.entry(v).or_insert(0) += 1;
                        }
                        gargs.iter().for_each(|a| cnt_expr(a, m));
                        hargs.iter().for_each(|a| cnt_expr(a, m));
                    } else {
                        let mut env = free_vars(bo);
                        env.remove(x);
                        for v in env {
                            *m.entry(v).or_insert(0) += 1;
                        }
                        gargs.iter().for_each(|a| cnt_expr(a, m));
                    }
                }
                Core::If(..) | Core::Match(..) => {
                    let mut env = free_vars(bo);
                    env.remove(x);
                    for v in env {
                        *m.entry(v).or_insert(0) += 1;
                    }
                    cnt_rule(r, m);
                }
                _ => unreachable!("codegen bug: non-normalized let RHS carrying a call"),
            }
        }
        Core::If(c, t, f) => {
            cnt_expr(c, m);
            let mut mt = Cnt::new();
            cnt_rule(t, &mut mt);
            let mut mf = Cnt::new();
            cnt_rule(f, &mut mf);
            merge_max(m, vec![mt, mf]);
        }
        Core::Match(s, arms) => {
            cnt_expr(s, m);
            let bs: Vec<Cnt> = arms
                .iter()
                .map(|(_, _, b)| {
                    let mut mm = Cnt::new();
                    cnt_rule(b, &mut mm);
                    mm
                })
                .collect();
            merge_max(m, bs);
        }
        Core::Call(_, xs) => xs.iter().for_each(|x| cnt_expr(x, m)),
        other => cnt_expr(other, m),
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
            let ev = ex.use_var(*v, true, b);
            b.push_str(&format!("{chn} = ctx.alloc({ev}, {chn}) as u64 + 1;\n"));
        }
    }
    let rn = ex.fresh();
    b.push_str(&format!(
        "let {rn} = ctx.alloc_rec({rule}u16, {pend}, {chn} as u32, 0, {par});\n"
    ));
    rn
}

fn emit_spawn(ex: &mut Ex, g: u32, args: &[Core], par: &str, b: &mut String) {
    let es: Vec<String> = args.iter().map(|a| ex.val(a, true, b)).collect();
    b.push_str(&format!("spawn_call(ctx, {}u16, &[{}], {});\n", 1 + g, es.join(", "), par));
}

/// Emit `e` (normalized) in rule-form tail position, delivering to `par`.
fn rtail(ex: &mut Ex, e: &Core, par: &str, b: &mut String, sq: &mut SegQ) {
    match e {
        Core::Let(x, r, bo) => {
            if !has_call(r) {
                let er = ex.val(r, false, b);
                if ex.rem.get(x).copied().unwrap_or(0) == 0 {
                    b.push_str(&format!("free_val(ctx, fr, {er});\n"));
                } else {
                    b.push_str(&format!("let v{x} = {er};\n"));
                }
                return rtail(ex, bo, par, b, sq);
            }
            match r.as_ref() {
                Core::Call(g, gargs) => {
                    // Pair fork: two adjacent independent calls share a record.
                    if let Some((y, h, hargs, bo2)) = fork_parts(x, bo) {
                        let mut env: BTreeSet<u32> = free_vars(bo2);
                        env.remove(x);
                        env.remove(y);
                        let env: Vec<u32> = env.into_iter().collect();
                        let sid = sq.add(vec![*x, *y], env.clone(), bo2.clone());
                        let rn = emit_rec(ex, &env, sid, 2, par, b);
                        emit_spawn(ex, *g, gargs, &format!("(({rn} as u64) << 3)"), b);
                        emit_spawn(ex, *h, hargs, &format!("((({rn} as u64) << 3) | 1)"), b);
                        return;
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
            let ec = ex.val(c, false, b);
            let saved = ex.rem.clone();
            b.push_str(&format!("if as_i({ec}) != 0 {{\n"));
            rtail(ex, t, par, b, sq);
            ex.rem = saved.clone();
            b.push_str("} else {\n");
            rtail(ex, f, par, b, sq);
            ex.rem = saved;
            b.push_str("}\n");
        }
        Core::Match(s, arms) => {
            let (sv, hold) = ex.scrutinee(s, b);
            let saved = ex.rem.clone();
            b.push_str(&format!("match con_tag({sv}) {{\n"));
            for (cid, binders, body) in arms {
                if *cid == UNREACHABLE_CTOR {
                    continue;
                }
                ex.rem = saved.clone();
                b.push_str(&format!("{cid} => {{\n"));
                ex.bind_fields(&sv, hold, binders, b);
                rtail(ex, body, par, b, sq);
                b.push_str("}\n");
            }
            ex.rem = saved;
            b.push_str("_ => { mith_unreachable(); }\n}\n");
        }
        Core::Call(g, args) => {
            emit_spawn(ex, *g, args, par, b);
        }
        other => {
            let v = ex.val(other, true, b);
            b.push_str(&format!("ctx.deliver({par}, {v});\n"));
        }
    }
}

/// The rule-form expansion of function `fid` (used when its dive unwinds):
/// evaluates the body event-driven from the original arguments (owned).
pub(crate) fn expand_fn(
    m: &CoreModule,
    fid: u32,
    body: &Core,
    bor: &[Vec<bool>],
    sq: &mut SegQ,
) -> String {
    let ar = m.fns[fid as usize].arity;
    let params: String = (0..ar).map(|i| format!(", v{i}: u64")).collect();
    let mut s = format!("fn x_{fid}(ctx: &mut Wctx, parent: u64{params}) {{\n");
    s.push_str("let fr = &mut Vec::new();\nlet al: &mut Vec<u32> = &mut Vec::new();\n");
    let mut rem = Cnt::new();
    cnt_rule(body, &mut rem);
    let mut ex = Ex::new(false, fid, false, rem, HashSet::new(), bor);
    let mut bb = String::new();
    for i in 0..ar as u32 {
        if ex.rem.get(&i).copied().unwrap_or(0) == 0 {
            bb.push_str(&format!("free_val(ctx, fr, v{i});\n"));
        }
    }
    rtail(&mut ex, body, "parent", &mut bb, sq);
    s.push_str(&bb);
    s.push_str("flush(ctx, fr);\n}\n\n");
    s
}

/// A continuation segment: fires when its record fills; slots arrive in
/// `e.a` / `e.b` (owned), the spilled environment is read back off the cell
/// chain (owned; the chain cells are freed as they are read).
pub(crate) fn segment_fn(m: &CoreModule, seg: &Seg, bor: &[Vec<bool>], sq: &mut SegQ) -> String {
    let _ = m;
    let mut s = format!("fn sg_{}(ctx: &mut Wctx, e: Redex) {{\n", seg.id);
    s.push_str("let fr = &mut Vec::new();\nlet al: &mut Vec<u32> = &mut Vec::new();\n");
    s.push_str("let inf = ctx.rec(e.aux as u32);\nlet parent = inf.parent;\n");
    let mut rem = Cnt::new();
    cnt_rule(&seg.body, &mut rem);
    s.push_str(&format!("let v{} = e.a;\n", seg.slots[0]));
    if seg.slots.len() > 1 {
        s.push_str(&format!("let v{} = e.b;\n", seg.slots[1]));
    }
    if !seg.env.is_empty() {
        s.push_str("let mut ch: u64 = inf.d as u64;\n");
        for v in &seg.env {
            s.push_str(&format!(
                "let v{v} = {{ let c = ctx.cell((ch - 1) as u32); ctx.free((ch - 1) as u32); ch = c[1]; c[0] }};\n"
            ));
        }
    }
    // Unused owned inputs die immediately.
    for sv in &seg.slots {
        if rem.get(sv).copied().unwrap_or(0) == 0 {
            s.push_str(&format!("free_val(ctx, fr, v{sv});\n"));
        }
    }
    let mut ex = Ex::new(false, u32::MAX, false, rem, HashSet::new(), bor);
    let mut bb = String::new();
    rtail(&mut ex, &seg.body, "parent", &mut bb, sq);
    s.push_str(&bb);
    s.push_str("flush(ctx, fr);\n}\n\n");
    s
}
