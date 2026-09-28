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
//! rule. Frees are deferred into a local `fr` and flushed when the fire
//! returns (fires are committed, so this is just batching).

use crate::lir::{c, do_, free, i64_, let_, rec_addr, set, truthy, u16_, u32_, u64_, cast, bin, v, Bop, FnDef, Inline, Ty, E, S};
use crate::seq::{vn, vparams, Ex};
use crate::{cnt_expr, free_vars, has_call, merge_max, Cnt};
use mithril_front::core::{Core, CoreModule};
use std::collections::BTreeSet;
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
    pub(crate) fn add(&mut self, fid: u32, slots: Vec<u32>, env: Vec<u32>, body: Core) -> u16 {
        let key = format!("{fid} {slots:?} {env:?} {body:?}");
        if let Some(&id) = self.memo.get(&key) {
            return id;
        }
        let id = self.next;
        self.next += 1;
        assert!(self.next != 0, "codegen: more than 65535 rules");
        self.memo.insert(key, id);
        self.q.push(Seg { id, fid, slots, env, body });
        id
    }

    pub(crate) fn add_hole(&mut self, cid: u32) -> u16 {
        if let Some((id, _)) = self.holes.iter().find(|(_, c)| *c == cid) {
            return *id;
        }
        let id = self.next;
        self.next += 1;
        assert!(self.next != 0, "codegen: more than 65535 rules");
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
        Core::App(f, a) => {
            let mut lets = Vec::new();
            let f2 = norm_pure(f, c, &mut lets);
            let a2 = norm_pure(a, c, &mut lets);
            wrap(lets, Core::App(Box::new(f2), Box::new(a2)))
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
        Core::App(f, a) => Core::App(Box::new(norm_pure(f, c, lets)), Box::new(norm_pure(a, c, lets))),
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
        Core::Call(..) | Core::If(..) | Core::Match(..) | Core::App(..) => {
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
        Core::Reuse(v, k, args) => {
            Core::Reuse(*v, *k, args.iter().map(|a| norm_pure(a, c, lets)).collect())
        }
        Core::Ctor(k, args) => {
            Core::Ctor(*k, args.iter().map(|a| norm_pure(a, c, lets)).collect())
        }
        Core::Prim(p, items) => Core::Prim(*p, items.iter().map(|a| norm_pure(a, c, lets)).collect()),
        Core::Tuple(items) => {
            Core::Tuple(items.iter().map(|a| norm_pure(a, c, lets)).collect())
        }
        Core::Proj(a, i) => Core::Proj(Box::new(norm_pure(a, c, lets)), *i),
        // a closure's body is not compiled here: it is built as a net
        Core::Lam(x, b) => Core::Lam(*x, b.clone()),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
    }
}

// ---- fork detection (shared by counting and emission) ----

/// `Let(x, Call g, Let(y, Call h, bo2))` with `h`'s args independent of `x`.
// ---- use counting for the rule form (mirrors rtail's structure) ----

pub(crate) fn cnt_rule(e: &Core, m: &mut Cnt) {
    match e {
        Core::Let(_, r, bo) => {
            // A call dives inline (continuation runs here) or suspends
            // (continuation moves into a record taking exactly its uses):
            // both paths consume the same counts.
            if has_call(r) {
                cnt_rule(r, m);
            } else {
                cnt_expr(r, m);
            }
            cnt_rule(bo, m);
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

/// Dives nested inline in one rule-form body before the rest is deferred
/// to a record (see `rtail`).
const MAX_INLINE_CALLS: u32 = 4;

/// The continuation record of `bo` after `x` (a one-slot segment).
fn cont_rec(ex: &mut Ex, x: u32, bo: &Core, par: &E, b: &mut Vec<S>, sq: &mut SegQ) -> String {
    let mut env = free_vars(bo);
    env.remove(&x);
    let env: Vec<u32> = env.into_iter().collect();
    let sid = sq.add(ex.self_fid, vec![x], env.clone(), bo.clone());
    emit_rec(ex, &env, sid, 1, par, b)
}

/// Emit `e` (normalized) in rule-form tail position, delivering to `par`.
fn rtail(ex: &mut Ex, e: &Core, par: &E, b: &mut Vec<S>, sq: &mut SegQ) {
    match e {
        Core::Let(x, r, bo) => {
            if !has_call(r) {
                ex.cur_let = Some(*x);
                let er = ex.val(r, false, b);
                ex.cur_let = None;
                if ex.rem.get(x).copied().unwrap_or(0) == 0 {
                    b.push(free(er));
                } else {
                    b.push(let_(vn(*x), Ty::U64, er));
                }
                return rtail(ex, bo, par, b, sq);
            }
            match r.as_ref() {
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
                        let rn = cont_rec(ex, *x, bo, par, b, sq);
                        es.insert(0, rec_addr(&rn));
                        b.push(do_(c("dive_to", vec![u16_(*g as u64), E::Slice(es)])));
                        return;
                    }
                    let saved = ex.rem.clone();
                    es.insert(0, E::Const("NONE".into()));
                    ex.inline_calls += 1;
                    let mut done = Vec::new();
                    if ex.rem.get(x).copied().unwrap_or(0) == 0 {
                        done.push(free(v("v")));
                    } else {
                        done.push(let_(vn(*x), Ty::U64, v("v")));
                    }
                    rtail(ex, bo, par, &mut done, sq);
                    ex.inline_calls -= 1;
                    ex.rem = saved.clone();
                    let mut susp = Vec::new();
                    if let Some((p_body, live, j_body)) = crate::seq::split_frame(*x, bo) {
                        let mut env: BTreeSet<u32> = free_vars(&j_body);
                        env.remove(x);
                        env.remove(&live);
                        let env: Vec<u32> = env.into_iter().collect();
                        let sid = sq.add(ex.self_fid, vec![*x, live], env.clone(), j_body.clone());
                        let rn = emit_rec(ex, &env, sid, 2, par, &mut susp);
                        susp.push(do_(c("set_parent", vec![v("rec"), rec_addr(&rn)])));
                        rtail(ex, &p_body, &bin(Bop::Or, rec_addr(&rn), u64_(1)), &mut susp, sq);
                    } else {
                        let rn = cont_rec(ex, *x, bo, par, &mut susp, sq);
                        susp.push(do_(c("set_parent", vec![v("rec"), rec_addr(&rn)])));
                    }
                    ex.rem = saved;
                    b.push(S::Res(c("dive_res", vec![u16_(*g as u64), E::Slice(es)]), "v".into(), done, "rec".into(), susp));
                }
                Core::If(..) | Core::Match(..) => {
                    let rn = cont_rec(ex, *x, bo, par, b, sq);
                    rtail(ex, r, &rec_addr(&rn), b, sq);
                }
                Core::App(f, a) => {
                    // a closure application runs in the net region: the
                    // continuation waits in a record the result is
                    // delivered to through a Kont port
                    let ef = ex.val(f, true, b);
                    let ea = ex.val(a, true, b);
                    let rn = cont_rec(ex, *x, bo, par, b, sq);
                    b.push(do_(c("apply_spawn", vec![ef, ea, rec_addr(&rn)])));
                }
                _ => unreachable!("codegen bug: non-normalized let RHS carrying a call"),
            }
        }
        Core::If(cd, t, f) => {
            let ec = ex.val(cd, false, b);
            let saved = ex.rem.clone();
            let live = free_vars(e);
            let (mut lt, mut lf) = (Cnt::new(), Cnt::new());
            cnt_rule(t, &mut lt);
            cnt_rule(f, &mut lf);
            let mut bt = Vec::new();
            ex.enter_branch(&live, &lt, &mut bt);
            rtail(ex, t, par, &mut bt, sq);
            ex.rem = saved.clone();
            let mut bf = Vec::new();
            ex.enter_branch(&live, &lf, &mut bf);
            rtail(ex, f, par, &mut bf, sq);
            ex.rem = saved;
            b.push(S::If(truthy(ec), bt, bf));
        }
        Core::Match(s, arms) => {
            let (sv, hold) = ex.scrutinee(s, b);
            let saved = ex.rem.clone();
            let mut live = free_vars(e);
            if let Core::Var(x) = &**s {
                live.remove(x);
            }
            let unbox = ex.unbox;
            let sw = crate::seq::plan_arms(&sv, arms, unbox, |i| {
                let (cid, binders, body) = &arms[i];
                ex.rem = saved.clone();
                let mut ab = Vec::new();
                let mut local = Cnt::new();
                cnt_rule(body, &mut local);
                ex.enter_branch(&live, &local, &mut ab);
                if unbox.contains_key(cid) {
                    if let Some(bv) = binders.first() {
                        if ex.rem.get(bv).copied().unwrap_or(0) > 0 {
                            ab.push(let_(vn(*bv), Ty::U64, crate::lir::num(crate::lir::as_i(sv.clone()))));
                        }
                    }
                } else {
                    ex.bind_fields(&sv, hold, *cid, binders, None, &mut ab);
                }
                rtail(ex, body, par, &mut ab, sq);
                ab
            });
            ex.rem = saved;
            b.push(sw);
        }
        Core::Call(g, args) => {
            let mut es: Vec<E> = args.iter().map(|a| ex.val(a, true, b)).collect();
            es.insert(0, par.clone());
            b.push(do_(c("dive_to", vec![u16_(*g as u64), E::Slice(es)])));
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

/// The rule-form expansion of function `fid` (used when its dive unwinds):
/// evaluates the body event-driven from the original arguments (owned).
pub(crate) fn expand_fn(
    m: &CoreModule,
    fid: u32,
    body: &Core,
    bor: &[Vec<bool>],
    sq: &mut SegQ,
    unbox: &std::collections::HashMap<u32, u8>,
    tys: &crate::ty::Types,
    iret: &[bool],
    shared: &std::cell::RefCell<crate::seq::Shared>,
) -> FnDef {
    let ar = m.fns[fid as usize].arity;
    let mut params = vec![("parent".to_string(), Ty::U64)];
    params.extend(vparams(ar));
    let mut rem = Cnt::new();
    cnt_rule(body, &mut rem);
    let ints = crate::ints_of(tys, fid as usize);
    let mut ex = Ex::new(false, fid, false, rem, HashSet::new(), bor, ints, None, unbox, iret, tys, shared);
    let mut bb = vec![fuel_local()];
    for i in 0..ar as u32 {
        if ex.rem.get(&i).copied().unwrap_or(0) == 0 {
            bb.push(free(v(vn(i))));
        }
    }
    rtail(&mut ex, body, &v("parent"), &mut bb, sq);
    FnDef { name: format!("x_{fid}"), ctx: true, params, ret: Ty::Unit, body: bb, inline: Inline::Default, cold: false }
}

/// The parameters of a rule function: the redex's two ports and its aux
/// word (the record index for record-activated rules).
pub(crate) fn rule_params() -> Vec<(String, Ty)> {
    vec![("a".into(), Ty::U64), ("b".into(), Ty::U64), ("aux".into(), Ty::U64)]
}

/// A continuation segment: fires when its record fills; slots arrive in
/// `a` / `b` (owned), the spilled environment is read back off the cell
/// chain (owned; the chain cells are freed as they are read).
pub(crate) fn segment_fn(m: &CoreModule, seg: &Seg, bor: &[Vec<bool>], sq: &mut SegQ, unbox: &std::collections::HashMap<u32, u8>, tys: &crate::ty::Types, iret: &[bool], shared: &std::cell::RefCell<crate::seq::Shared>) -> FnDef {
    let _ = m;
    let mut s = vec![
        S::Comment(format!("segment of fn {} slots {:?} env {:?}", seg.fid, seg.slots, seg.env)),
        fuel_local(),
        let_("parent", Ty::U64, c("rec_parent", vec![cast(v("aux"), Ty::U32)])),
    ];
    let mut rem = Cnt::new();
    cnt_rule(&seg.body, &mut rem);
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
    for sv in &seg.slots {
        if rem.get(sv).copied().unwrap_or(0) == 0 {
            s.push(free(v(vn(*sv))));
        }
    }
    let ints = if seg.fid == u32::MAX {
        HashSet::new()
    } else {
        crate::ints_of(tys, seg.fid as usize)
    };
    let mut ex = Ex::new(false, seg.fid, false, rem, HashSet::new(), bor, ints, None, unbox, iret, tys, shared);
    rtail(&mut ex, &seg.body, &v("parent"), &mut s, sq);
    FnDef { name: format!("sg_{}", seg.id), ctx: true, params: rule_params(), ret: Ty::Unit, body: s, inline: Inline::Default, cold: false }
}
