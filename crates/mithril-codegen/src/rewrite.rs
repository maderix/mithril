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
    tail(body, &cx, &mut avail)
}

fn take_token(avail: &mut Vec<(u32, usize)>, arity: usize) -> Option<u32> {
    let pos = avail.iter().rposition(|(_, a)| *a == arity)?;
    Some(avail.remove(pos).0)
}

/// Value position: a construct may take a token; calls and branches end
/// the straight-line span (branches evaluate with no tokens inside).
fn val(e: &Core, cx: &ReuseCtx, avail: &mut Vec<(u32, usize)>) -> Core {
    match e {
        Core::Ctor(c, xs) => {
            let xs2: Vec<Core> = xs.iter().map(|x| val(x, cx, avail)).collect();
            let ar = cx.m.ctors[*c as usize].1;
            if ar == 2 && !cx.unbox.contains_key(c) {
                if let Some(v) = take_token(avail, 2) {
                    return Core::Reuse(v, *c, xs2);
                }
            }
            Core::Ctor(*c, xs2)
        }
        Core::Call(g, xs) => {
            let xs2 = xs.iter().map(|x| val(x, cx, avail)).collect();
            avail.clear();
            Core::Call(*g, xs2)
        }
        Core::If(c, t, f) => {
            let c2 = val(c, cx, avail);
            let mut none = Vec::new();
            let t2 = val(t, cx, &mut none);
            none.clear();
            let f2 = val(f, cx, &mut none);
            avail.clear();
            Core::If(Box::new(c2), Box::new(t2), Box::new(f2))
        }
        Core::Match(s, arms) => {
            let s2 = val(s, cx, avail);
            let mut none = Vec::new();
            let arms2 = arms
                .iter()
                .map(|(c, bs, b)| {
                    none.clear();
                    (*c, bs.clone(), val(b, cx, &mut none))
                })
                .collect();
            avail.clear();
            Core::Match(Box::new(s2), arms2)
        }
        Core::Let(x, r, b) => {
            let r2 = val(r, cx, avail);
            let b2 = val(b, cx, avail);
            Core::Let(*x, Box::new(r2), Box::new(b2))
        }
        Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(val(a, cx, avail)), Box::new(val(b, cx, avail))),
        Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(val(a, cx, avail)), Box::new(val(b, cx, avail))),
        Core::Tuple(xs) => Core::Tuple(xs.iter().map(|x| val(x, cx, avail)).collect()),
        Core::Prim(p, xs) => Core::Prim(*p, xs.iter().map(|x| val(x, cx, avail)).collect()),
        Core::Proj(b, i) => Core::Proj(Box::new(val(b, cx, avail)), *i),
        // a closure body is another scope: no reuse token crosses into it
        Core::Lam(x, b) => Core::Lam(*x, Box::new(val(b, cx, &mut Vec::new()))),
        Core::App(f, a) => Core::App(Box::new(val(f, cx, avail)), Box::new(val(a, cx, avail))),
        Core::Reuse(v, c, xs) => Core::Reuse(*v, *c, xs.iter().map(|x| val(x, cx, avail)).collect()),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => e.clone(),
    }
}

/// Tail position: branches inherit the tokens (each path independently).
fn tail(e: &Core, cx: &ReuseCtx, avail: &mut Vec<(u32, usize)>) -> Core {
    match e {
        Core::Let(x, r, b) => {
            let r2 = val(r, cx, avail);
            let b2 = tail(b, cx, avail);
            Core::Let(*x, Box::new(r2), Box::new(b2))
        }
        Core::If(c, t, f) => {
            let c2 = val(c, cx, avail);
            let mut a1 = avail.clone();
            let t2 = tail(t, cx, &mut a1);
            let mut a2 = avail.clone();
            let f2 = tail(f, cx, &mut a2);
            avail.clear();
            Core::If(Box::new(c2), Box::new(t2), Box::new(f2))
        }
        Core::Match(s, arms) => {
            let s2 = val(s, cx, avail);
            // a consumed boxed scrutinee cell becomes a token for each arm
            let tok = match &**s {
                Core::Var(v) if cx.uses.get(v) == Some(&1) => Some(*v),
                _ => None,
            };
            let arms2 = arms
                .iter()
                .map(|(c, bs, b)| {
                    let mut a = avail.clone();
                    if let Some(v) = tok.filter(|_| *c != UNREACHABLE_CTOR && !cx.unbox.contains_key(c) && cx.m.ctors[*c as usize].1 == 2) {
                        a.push((v, 2));
                    }
                    (*c, bs.clone(), tail(b, cx, &mut a))
                })
                .collect();
            avail.clear();
            Core::Match(Box::new(s2), arms2)
        }
        other => val(other, cx, avail),
    }
}
