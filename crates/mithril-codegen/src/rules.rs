//! Rule-form lowering: ANF normalization plus event-driven compilation of a
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
//! rule.

use crate::lir::{c, do_, i64_, let_, rec_addr, set, u16_, u32_, u64_, cast, bin, v, Bop, FnDef, Inline, Ty, E, S};
use crate::seq::{vn, Ex};
use crate::{cnt_rule, free_vars, has_call};
use mithril_front::core::Core;
use std::collections::HashSet;

#[derive(Clone)]
pub(crate) struct Seg {
    pub id: u16,
    /// Owning function (for var typing); u32::MAX = none (forwarding seg).
    pub fid: u32,
    pub slots: Vec<u32>,
    pub env: Vec<u32>,
    pub body: Core,
}

pub(crate) struct SegQ {
    pub next: u16,
    pub q: Vec<Seg>,
    /// The same continuation is reached from every path that suspends at
    /// its call site (the dive form, and each segment whose inline path
    /// runs that call); it gets one segment, not one per path.
    pub memo: std::collections::HashMap<String, u16>,
    /// Hole-fill rules of TRMC functions: (rule id, ctor id). The record
    /// holds the head cell (`d`) and the pending hole cell (`s`); its one
    /// input is the value for the hole.
    pub holes: Vec<(u16, u32)>,
}

impl SegQ {
    fn alloc(&mut self) -> u16 {
        let id = self.next;
        self.next += 1;
        assert!(self.next != 0, "codegen: more than 65535 rules");
        id
    }

    pub(crate) fn add(&mut self, fid: u32, slots: Vec<u32>, env: Vec<u32>, body: Core) -> u16 {
        let key = format!("{fid} {slots:?} {env:?} {body:?}");
        if let Some(&id) = self.memo.get(&key) {
            return id;
        }
        let id = self.alloc();
        self.memo.insert(key, id);
        self.q.push(Seg { id, fid, slots, env, body });
        id
    }

    pub(crate) fn add_hole(&mut self, cid: u32) -> u16 {
        if let Some((id, _)) = self.holes.iter().find(|(_, c)| *c == cid) {
            return *id;
        }
        let id = self.alloc();
        self.holes.push((id, cid));
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

/// Normalize a function body (tail position): its prerequisite bindings
/// wrap the let-RHS shape of the body.
pub(crate) fn normalize(e: &Core, c: &mut u32) -> Core {
    let mut lets = Vec::new();
    let t = norm(e, c, &mut lets, false);
    wrap(lets, t)
}

/// Normalize a let RHS (`pure = false`) or a call-free value position.
/// Hoisting, branch tails, and lambda scope boundaries share one traversal.
fn norm(e: &Core, c: &mut u32, lets: &mut Vec<(u32, Core)>, pure: bool) -> Core {
    if !has_call(e) {
        return e.clone();
    }
    if pure && matches!(e, Core::Call(..) | Core::If(..) | Core::Match(..) | Core::App(..)) {
        let rhs = norm(e, c, lets, false);
        let x = fresh(c);
        lets.push((x, rhs));
        return Core::Var(x);
    }
    match e {
        Core::Let(x, rhs, body) => {
            let rhs = norm(rhs, c, lets, false);
            lets.push((*x, rhs));
            norm(body, c, lets, pure)
        }
        Core::If(..) | Core::Match(..) => {
            // Condition first, followed by independently normalized tail bodies.
            let mut first = true;
            crate::rewrite::map_children(e, &mut |child| {
                if std::mem::replace(&mut first, false) {
                    norm(child, c, lets, true)
                } else {
                    normalize(child, c)
                }
            })
        }
        Core::Lam(x, body) => Core::Lam(*x, body.clone()),
        _ => crate::rewrite::map_children(e, &mut |child| norm(child, c, lets, true)),
    }
}

// ---- rule-form emission ----

/// Allocate a waiting record for segment `rule` with the given spilled
/// environment; returns the record temp name.
pub(crate) fn emit_rec(ex: &mut Ex, env: &[u32], rule: u16, pend: u32, par: &E, b: &mut Vec<S>) -> String {
    let chn = ex.fresh();
    b.push(let_(&chn, Ty::U64, u64_(0)));
    for x in env.iter().rev() {
        ex.captured.insert(*x);
        let ev = ex.use_var(*x, true, b);
        b.push(set(&chn, bin(Bop::Add, cast(c("alloc2", vec![ev, v(&chn)]), Ty::U64), u64_(1))));
    }
    let rn = ex.fresh();
    b.push(let_(&rn, Ty::U32, c("alloc_rec", vec![u16_(rule as u64), u32_(pend as u64), cast(v(&chn), Ty::U32), u32_(0), par.clone()])));
    rn
}

/// A record for segment `body` waiting on `slots` (pend = their number);
/// the rest of its free variables (sorted) spill into the env chain.
pub(crate) fn wait_rec(ex: &mut Ex, sq: &mut SegQ, slots: Vec<u32>, body: Core, par: &E, b: &mut Vec<S>) -> String {
    let env: Vec<u32> = free_vars(&body).into_iter().filter(|v| !slots.contains(v)).collect();
    let pend = slots.len() as u32;
    let sid = sq.add(ex.self_fid, slots, env.clone(), body);
    emit_rec(ex, &env, sid, pend, par, b)
}

/// The records a split frame's dependent rest waits in: the pend-2 join
/// `rn` and the record x is delivered to (a pend-1 D under the join when a
/// call needs only x, else the join itself).
pub(crate) fn join_records(ex: &mut Ex, sq: &mut SegQ, x: u32, live: u32, j_body: &Core, par: &E, b: &mut Vec<S>) -> (String, String) {
    match crate::seq::split_dep(x, live, j_body) {
        Some((d_body, m, j2)) => {
            let rn = wait_rec(ex, sq, vec![m, live], j2, par, b);
            let rd = wait_rec(ex, sq, vec![x], d_body, &rec_addr(&rn), b);
            (rn, rd)
        }
        None => {
            let rn = wait_rec(ex, sq, vec![x, live], j_body.clone(), par, b);
            (rn.clone(), rn)
        }
    }
}

/// Dives nested inline in one rule-form body before the rest is deferred
/// to a record (see `rtail`).
const MAX_INLINE_CALLS: u32 = 4;

/// Emit `e` (normalized) in rule-form tail position, delivering to `par`.
fn rtail(ex: &mut Ex, e: &Core, par: &E, b: &mut Vec<S>, sq: &mut SegQ) {
    match e {
        Core::Let(x, r, bo) => {
            if !has_call(r) {
                ex.bind_val(*x, r, b);
                return rtail(ex, bo, par, b, sq);
            }
            match r.as_ref() {
                // `let x = g(..) in x`: a tail call
                Core::Call(..) if matches!(&**bo, Core::Var(y) if y == x) => rtail(ex, r, par, b, sq),
                Core::Call(g, gargs) => {
                    // Dive inline; the continuation runs right here when the
                    // callee finishes within fuel. On suspension the frame
                    // splits like the dive form's capture: the independent
                    // suffix P runs now (inline, delivering to the join's
                    // second slot), the dependent rest J waits in a record.
                    let mut es: Vec<E> = gargs.iter().map(|a| ex.val(a, true, b)).collect();
                    if ex.inline_calls >= MAX_INLINE_CALLS {
                        // Deep in a call chain: the continuation waits in a
                        // record (its own memoized segment) instead of being
                        // nested inline again — code stays linear in the
                        // chain length, and a finished dive just delivers.
                        let rn = wait_rec(ex, sq, vec![*x], (**bo).clone(), par, b);
                        es.insert(0, rec_addr(&rn));
                        b.push(do_(c("dive_to", vec![u16_(*g as u64), E::Slice(es)])));
                        return;
                    }
                    let saved = ex.rem.clone();
                    es.insert(0, E::Const("NONE".into()));
                    ex.inline_calls += 1;
                    let mut done = Vec::new();
                    ex.emit_bind(*x, v("v"), &mut done);
                    rtail(ex, bo, par, &mut done, sq);
                    ex.inline_calls -= 1;
                    ex.rem = saved.clone();
                    let mut susp = Vec::new();
                    let split = crate::seq::split_frame(*x, bo);
                    let fork = split.is_some();
                    if let Some((p_body, live, j_body)) = split {
                        let (rn, rx) = join_records(ex, sq, *x, live, &j_body, par, &mut susp);
                        susp.push(do_(c("set_parent", vec![v("rec"), rec_addr(&rx)])));
                        rtail(ex, &p_body, &bin(Bop::Or, rec_addr(&rn), u64_(1)), &mut susp, sq);
                    } else {
                        let rn = wait_rec(ex, sq, vec![*x], (**bo).clone(), par, &mut susp);
                        susp.push(do_(c("set_parent", vec![v("rec"), rec_addr(&rn)])));
                    }
                    ex.rem = saved;
                    let dive = if fork { "dive_res_fork" } else { "dive_res" };
                    b.push(S::Res(c(dive, vec![u16_(*g as u64), E::Slice(es)]), "v".into(), done, "rec".into(), susp));
                }
                Core::If(..) | Core::Match(..) => {
                    let rn = wait_rec(ex, sq, vec![*x], (**bo).clone(), par, b);
                    rtail(ex, r, &rec_addr(&rn), b, sq);
                }
                Core::App(f, a) => {
                    // a closure application runs in the net region: the
                    // continuation waits in a record the result is
                    // delivered to through a Kont port
                    let ef = ex.val(f, true, b);
                    let ea = ex.val(a, true, b);
                    let rn = wait_rec(ex, sq, vec![*x], (**bo).clone(), par, b);
                    b.push(do_(c("apply_spawn", vec![ef, ea, rec_addr(&rn)])));
                }
                _ => unreachable!("codegen bug: non-normalized let RHS carrying a call"),
            }
        }
        Core::If(cd, t, f) => ex.if_arms(e, cd, t, f, cnt_rule, |ex, body, ab| rtail(ex, body, par, ab, sq), b),
        Core::Match(s, arms) => ex.match_arms(e, s, arms, cnt_rule, |ex, body, ab| rtail(ex, body, par, ab, sq), b),
        Core::Call(g, args) => {
            // a tail call delivering to `par`: in the parallel world the
            // device runtime spawns it as a task (a marked call);
            // otherwise it dives here
            let mut es: Vec<E> = args.iter().map(|a| ex.val(a, true, b)).collect();
            es.insert(0, par.clone());
            b.push(do_(c("tail_to", vec![u16_(*g as u64), E::Slice(es)])));
        }
        Core::App(f, a) => {
            let ef = ex.val(f, true, b);
            let ea = ex.val(a, true, b);
            b.push(do_(c("apply_spawn", vec![ef, ea, par.clone()])));
        }
        other => {
            let x = ex.val(other, true, b);
            b.push(do_(c("deliver", vec![par.clone(), x])));
        }
    }
}

/// The rule form's dead fuel local (bounded callees never read it).
fn fuel_local() -> S {
    let_("fl0", Ty::I64, i64_(0))
}

/// The parameters of a rule function: the redex's two ports and its aux
/// word (the record index for record-activated rules).
pub(crate) fn rule_params() -> Vec<(String, Ty)> {
    vec![("a".into(), Ty::U64), ("b".into(), Ty::U64), ("aux".into(), Ty::U64)]
}

/// A continuation segment: fires when its record fills; slots arrive in
/// `a` / `b` (owned), the spilled environment is read back off the cell
/// chain (owned; the chain cells are freed as they are read).
pub(crate) fn segment_fn(seg: &Seg, bor: &[Vec<bool>], sq: &mut SegQ, unbox: &std::collections::HashMap<u32, u8>, tys: &crate::ty::Types, iret: &[bool], shared: &std::cell::RefCell<crate::seq::Shared>) -> FnDef {
    let mut s = vec![
        S::Comment(format!("segment of fn {} slots {:?} env {:?}", seg.fid, seg.slots, seg.env)),
        fuel_local(),
        let_("parent", Ty::U64, c("rec_parent", vec![cast(v("aux"), Ty::U32)])),
    ];
    let mut ex = Ex::new(false, seg.fid, false, &seg.body, HashSet::new(), bor, None, unbox, iret, tys, shared);
    if !seg.slots.is_empty() {
        s.push(let_(vn(seg.slots[0]), Ty::U64, v("a")));
    }
    if seg.slots.len() > 1 {
        s.push(let_(vn(seg.slots[1]), Ty::U64, v("b")));
    }
    if !seg.env.is_empty() {
        s.push(let_("ch", Ty::U64, cast(c("rec_d", vec![cast(v("aux"), Ty::U32)]), Ty::U64)));
        for x in &seg.env {
            s.push(let_(vn(*x), Ty::U64, c("pop_chain", vec![E::Ref("ch".into())])));
        }
    }
    // Unused owned inputs die immediately.
    ex.free_dead(seg.slots.iter().copied(), &mut s);
    rtail(&mut ex, &seg.body, &v("parent"), &mut s, sq);
    FnDef { name: format!("sg_{}", seg.id), ctx: true, params: rule_params(), ret: Ty::Unit, body: s, inline: Inline::Default, cold: false }
}

#[cfg(test)]
#[path = "../tests/support/normalization.rs"]
mod normalization_tests;
