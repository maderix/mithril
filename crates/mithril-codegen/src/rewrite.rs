//! Static Core → Core rewrites that run before emission. Each is a
//! semantics-preserving transformation on the IR (checkable against
//! `eval_core`); the emitter only materializes what they decided.
//!
//! Constant folding, inlining and static unfolding are not here: they are
//! interaction-rule firings in `mithril_net::specialize`, which runs before
//! codegen; this file holds the rewrites codegen itself owns.
//! * `mark_reuse`: a constructor built on a call-free straight-line path
//!   after a match consumed a same-arity constructor cell is rewritten to
//!   `Reuse(v, c, args)`: build in the dead cell instead of allocating. The
//!   token never crosses a call, a value-position branch, or a loop
//!   back-edge (the free-early rule: the cell returns to the LIFO free list
//!   before other work runs), and every path that does not reuse a token
//!   releases it at its terminal (the emitter emits that release).

use crate::has_call;
use mithril_front::core::{Core, CoreModule, UNREACHABLE_CTOR};
use std::collections::HashMap;

/// Tail inlining: a small, non-self-recursive `g` tail-called from `f`
/// whose own calls are tail calls back to `f` (or calls to call-free
/// functions) is inlined at that site. Mutual tail recursion then becomes
/// self tail recursion (a loop); semantics are unchanged (pure bodies).
pub(crate) fn tail_inline(m: &CoreModule) -> CoreModule {
    const MAX_SIZE: usize = 64;
    let leaf: Vec<bool> = m.fns.iter().map(|f| !has_call(&f.body)).collect();
    // g qualifies for inlining into f's tail sites
    let fits = |f: u32, g: u32| -> bool {
        if f == g || g == m.main {
            return false;
        }
        let gb = &m.fns[g as usize].body;
        if gb.size() > MAX_SIZE {
            return false;
        }
        let cs = crate::call_sites(gb);
        cs.contains(&f)
            && cs.iter().all(|h| *h == f || (*h != g && leaf[*h as usize]))
            && mithril_front::desugar::compute_self_tail_rec(f, gb)
    };
    let mut out = m.clone();
    for (fid, f) in out.fns.iter_mut().enumerate() {
        let mut next = f.body.max_var().max(f.arity as u32) + 1;
        let nb = map_tails(&f.body, &mut |e| match e {
            Core::Call(g, args) if fits(fid as u32, *g) => {
                let callee = &m.fns[*g as usize];
                let base = next;
                next += callee.arity as u32;
                let shift = next;
                next += callee.body.max_var().max(callee.arity as u32) + 1;
                let mut map = HashMap::new();
                for p in 0..callee.arity as u32 {
                    match &args[p as usize] {
                        Core::Var(y) => map.insert(p, *y),
                        _ => map.insert(p, base + p),
                    };
                }
                let mut out = shift_binders(&callee.body, shift, callee.arity as u32, &map);
                for (p, a) in args.iter().enumerate().rev() {
                    if !matches!(a, Core::Var(_)) {
                        out = Core::Let(base + p as u32, Box::new(a.clone()), Box::new(out));
                    }
                }
                Some(out)
            }
            _ => None,
        });
        if nb != f.body {
            f.body = nb;
            f.self_tail_rec = mithril_front::desugar::compute_self_tail_rec(fid as u32, &f.body);
        }
    }
    out
}

/// `e` with `f` applied at its tail positions (`None`: descend through
/// Let/If/Match; any other tail stays).
fn map_tails(e: &Core, f: &mut dyn FnMut(&Core) -> Option<Core>) -> Core {
    if let Some(r) = f(e) {
        return r;
    }
    match e {
        Core::Let(x, r, b) => Core::Let(*x, r.clone(), Box::new(map_tails(b, f))),
        Core::If(c, t, el) => Core::If(c.clone(), Box::new(map_tails(t, f)), Box::new(map_tails(el, f))),
        Core::Match(s, arms) => Core::Match(s.clone(), arms.iter().map(|(c, bs, b)| (*c, bs.clone(), map_tails(b, f))).collect()),
        other => other.clone(),
    }
}

/// Rename every binder of a closed body: params via `map`, locals to
/// `shift + old`.
fn shift_binders(e: &Core, shift: u32, arity: u32, map: &HashMap<u32, u32>) -> Core {
    e.rename(&mut |i| if i < arity { map[&i] } else { shift + i })
}

// ---------------------------------------------------------------- reuse

fn count_uses(e: &Core, m: &mut HashMap<u32, u32>) {
    e.walk(&mut |e| match e {
        Core::Var(i) | Core::Reuse(i, _, _) => *m.entry(*i).or_insert(0) += 1,
        _ => {}
    });
}

pub(crate) struct ReuseCtx<'a> {
    pub m: &'a CoreModule,
    pub unbox: &'a HashMap<u32, u8>,
    pub uses: HashMap<u32, u32>,
}

/// Mark reuse on a normalized, uniquified body (tail position entry).
pub(crate) fn mark_reuse(body: &Core, m: &CoreModule, unbox: &HashMap<u32, u8>) -> Core {
    let mut uses = HashMap::new();
    count_uses(body, &mut uses);
    let cx = ReuseCtx { m, unbox, uses };
    let mut avail: Vec<(u32, usize)> = Vec::new();
    reuse(body, &cx, &mut avail, true)
}

fn take_token(avail: &mut Vec<(u32, usize)>, arity: usize) -> Option<u32> {
    let pos = avail.iter().rposition(|(_, a)| *a == arity)?;
    Some(avail.remove(pos).0)
}

/// Rebuild immediate children in Core evaluation order. Binder metadata is
/// untouched; the caller chooses which scopes and branches to recurse into.
pub(crate) fn map_children(e: &Core, f: &mut dyn FnMut(&Core) -> Core) -> Core {
    let mut out = e.clone();
    match &mut out {
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) | Core::App(a, b) => { **a = f(a); **b = f(b); }
        Core::If(a, b, c) => { **a = f(a); **b = f(b); **c = f(c); }
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Reuse(_, _, xs) | Core::Tuple(xs) | Core::Prim(_, xs) => for x in xs { *x = f(x); },
        Core::Match(s, arms) => { **s = f(s); for (_, _, b) in arms { *b = f(b); } }
        Core::Proj(a, _) | Core::Lam(_, a) => **a = f(a),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
    }
    out
}

/// One reuse walk. Tail branches inherit tokens; value branches start empty.
/// Calls clear the span, and lambda bodies always start in their own scope.
fn reuse(e: &Core, cx: &ReuseCtx, avail: &mut Vec<(u32, usize)>, tail: bool) -> Core {
    match e {
        Core::Ctor(c, xs) => {
            let xs: Vec<Core> = xs.iter().map(|x| reuse(x, cx, avail, false)).collect();
            if cx.m.ctors[*c as usize].1 == 2 && !cx.unbox.contains_key(c) {
                if let Some(v) = take_token(avail, 2) { return Core::Reuse(v, *c, xs); }
            }
            Core::Ctor(*c, xs)
        }
        Core::Call(..) => {
            let call = map_children(e, &mut |x| reuse(x, cx, avail, false));
            avail.clear();
            call
        }
        Core::Let(x, r, b) => Core::Let(*x, Box::new(reuse(r, cx, avail, false)), Box::new(reuse(b, cx, avail, tail))),
        Core::If(c, t, f) => {
            let c = reuse(c, cx, avail, false);
            let seed = if tail { avail.clone() } else { Vec::new() };
            let t = reuse(t, cx, &mut seed.clone(), tail);
            let f = reuse(f, cx, &mut seed.clone(), tail);
            avail.clear();
            Core::If(Box::new(c), Box::new(t), Box::new(f))
        }
        Core::Match(s, arms) => {
            let s2 = reuse(s, cx, avail, false);
            // Only a tail match can lend its consumed spine to an arm.
            let tok = match &**s { Core::Var(v) if tail && cx.uses.get(v) == Some(&1) => Some(*v), _ => None };
            let seed = if tail { avail.clone() } else { Vec::new() };
            let arms = arms.iter().map(|(c, bs, b)| {
                let mut a = seed.clone();
                if let Some(v) = tok.filter(|_| *c != UNREACHABLE_CTOR && !cx.unbox.contains_key(c) && cx.m.ctors[*c as usize].1 == 2) { a.push((v, 2)); }
                (*c, bs.clone(), reuse(b, cx, &mut a, tail))
            }).collect();
            avail.clear();
            Core::Match(Box::new(s2), arms)
        }
        Core::Lam(x, b) => Core::Lam(*x, Box::new(reuse(b, cx, &mut Vec::new(), false))),
        _ => map_children(e, &mut |x| reuse(x, cx, avail, false)),
    }
}

#[cfg(test)]
#[path = "../tests/support/core_rebuild.rs"]
mod rebuild_tests;
