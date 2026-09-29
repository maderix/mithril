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

// ---- if-conversion of same-call branches ----
//
// An if-tree in tail position whose every leaf is (pure bindings; call g)
// with pure arguments becomes one call to g whose differing arguments are
// selects, with the arms' bindings and the conditions computed up front.
// Typical source: a loop body ending in if/elif that updates different
// accumulators; each branch is the same back-edge. A branch on runtime
// data costs a misprediction per iteration, a select does not. Everything
// hoisted is pure and cannot fault (no calls; division only by nonzero
// constants), so computing it speculatively preserves the semantics.

const IFCONV_LETS: usize = 128;
/// Operations the whole if-tree may speculate (every arm's work is done
/// on every iteration once converted): a branch costs about a
/// misprediction per iteration, so only a tree cheaper than that is
/// worth it. Measured in operations, not bindings: the let-structure is
/// the residual reader's choice, the work is not.
const IFCONV_WORK: usize = 32;

fn pure_nofault(e: &Core) -> bool {
    use mithril_front::ast::BinOp::*;
    match e {
        Core::Num(_) | Core::Var(_) => true,
        Core::Op2(op, a, b) => {
            let div_ok = !matches!(op, Div | FloorDiv | Mod) || matches!(**b, Core::Num(n) if n != 0);
            div_ok && pure_nofault(a) && pure_nofault(b)
        }
        Core::Cmp(_, a, b) => pure_nofault(a) && pure_nofault(b),
        Core::If(c, t, f) => pure_nofault(c) && pure_nofault(t) && pure_nofault(f),
        _ => false,
    }
}

/// A leaf of a qualifying tree: its path (hoisted condition var, taken
/// branch) and its call arguments.
type Leaf = (Vec<(u32, bool)>, Vec<Core>);

/// (hoisted bindings, condition var -> condition, callee, leaves).
fn ifconv(
    e: &Core,
    next: &mut u32,
    budget: &mut usize,
    path: &mut Vec<(u32, bool)>,
    conds: &mut HashMap<u32, Core>,
) -> Option<(Vec<(u32, Core)>, u32, Vec<Leaf>)> {
    match e {
        Core::Let(x, r, b) if pure_nofault(r) => {
            *budget = budget.checked_sub(1)?;
            let (mut lets, g, leaves) = ifconv(b, next, budget, path, conds)?;
            lets.insert(0, (*x, (**r).clone()));
            Some((lets, g, leaves))
        }
        Core::Call(g, args) if args.iter().all(pure_nofault) => Some((Vec::new(), *g, vec![(path.clone(), args.clone())])),
        Core::If(c, t, f) if pure_nofault(c) => {
            *budget = budget.checked_sub(1)?;
            let cv = *next;
            *next += 1;
            conds.insert(cv, (**c).clone());
            path.push((cv, true));
            let (lt, g1, mut at) = ifconv(t, next, budget, path, conds)?;
            path.pop();
            path.push((cv, false));
            let (lf, g2, af) = ifconv(f, next, budget, path, conds)?;
            path.pop();
            if g1 != g2 || at.iter().chain(&af).any(|(_, a)| a.len() != at[0].1.len()) {
                return None;
            }
            let mut lets = vec![(cv, (**c).clone())];
            lets.extend(lt);
            lets.extend(lf);
            at.extend(af);
            Some((lets, g1, at))
        }
        _ => None,
    }
}

/// `v == k` for the condition bound to `cv`.
fn eq_test(conds: &HashMap<u32, Core>, cv: u32) -> Option<(u32, i64)> {
    match conds.get(&cv)? {
        Core::Cmp(mithril_front::ast::CmpOp::Eq, a, b) => match (&**a, &**b) {
            (Core::Var(v), Core::Num(k)) | (Core::Num(k), Core::Var(v)) => Some((*v, *k)),
            _ => None,
        },
        _ => None,
    }
}

/// The 0/1 condition of reaching a leaf along `path`. In a chain of
/// equality tests of one variable against distinct constants, reaching a
/// taken test implies every earlier test failed: its own test suffices.
fn path_cond(path: &[(u32, bool)], conds: &HashMap<u32, Core>) -> Core {
    if let Some((last, true)) = path.last() {
        if let Some((v, k)) = eq_test(conds, *last) {
            let mut ks = vec![k];
            let chain = path[..path.len() - 1].iter().all(|(cv, taken)| {
                !taken && eq_test(conds, *cv).is_some_and(|(w, j)| {
                    ks.push(j);
                    w == v
                })
            });
            ks.sort_unstable();
            ks.dedup();
            if chain && ks.len() == path.len() {
                return Core::Var(*last);
            }
        }
    }
    let lit = |(cv, taken): &(u32, bool)| {
        if *taken {
            Core::Var(*cv)
        } else {
            Core::Op2(mithril_front::ast::BinOp::BitXor, Box::new(Core::Var(*cv)), Box::new(Core::Num(1)))
        }
    };
    let mut it = path.iter();
    let first = it.next().map(lit).unwrap_or(Core::Num(1));
    it.fold(first, |acc, l| Core::Op2(mithril_front::ast::BinOp::BitAnd, Box::new(acc), Box::new(lit(l))))
}

/// The select for one argument position across the leaves.
fn select_arg(leaves: &[Leaf], j: usize, conds: &HashMap<u32, Core>) -> Core {
    let first = &leaves[0].1[j];
    if leaves.iter().all(|(_, a)| a[j] == *first) {
        return first.clone();
    }
    // one leaf differs from a value shared by all the others
    for (i, (p, a)) in leaves.iter().enumerate() {
        let others: Vec<&Core> = leaves.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, (_, b))| &b[j]).collect();
        if others.iter().all(|o| **o == *others[0]) && *others[0] != a[j] {
            return Core::If(Box::new(path_cond(p, conds)), Box::new(a[j].clone()), Box::new(others[0].clone()));
        }
    }
    // general case: nest by leaf path conditions
    let (last, rest) = leaves.split_last().unwrap();
    rest.iter().rev().fold(last.1[j].clone(), |acc, (p, a)| {
        Core::If(Box::new(path_cond(p, conds)), Box::new(a[j].clone()), Box::new(acc))
    })
}

/// Operations an if-tree computes once converted: all of its arms.
fn tree_ops(e: &Core) -> usize {
    e.sum(&mut |e| matches!(e, Core::Op2(..) | Core::Cmp(..) | Core::Proj(..) | Core::Lam(..) | Core::App(..)) as usize)
}

/// Apply if-conversion at every tail-position if-tree of every function
/// whose leaves are the function's own loop back-edge (a self tail call)
/// and whose arms are cheap: speculating a heavy arm, or converting a
/// predictable branch outside a loop, costs more than the branch.
pub(crate) fn if_convert(m: &CoreModule) -> CoreModule {
    let tys = crate::ty::infer(m);
    // selects are for plain ints: choosing between boxed values would
    // need reference copies of both candidates
    fn int_expr(e: &Core, fid: u32, tys: &crate::ty::Types) -> bool {
        match e {
            Core::Num(_) | Core::Cmp(..) => true,
            Core::Var(v) => tys.var(fid as usize, *v) == crate::ty::Ty::Int,
            Core::Op2(_, a, b) => int_expr(a, fid, tys) && int_expr(b, fid, tys),
            Core::If(_, t, f) => int_expr(t, fid, tys) && int_expr(f, fid, tys),
            _ => false,
        }
    }
    let mut out = m.clone();
    for (fid, f) in out.fns.iter_mut().enumerate() {
        let fid = fid as u32;
        let mut next = f.body.max_var().max(f.arity as u32) + 1;
        f.body = map_tails(&f.body, &mut |e| {
            let Core::If(..) = e else { return None };
            let mut budget = IFCONV_LETS;
            let mut n2 = next;
            let mut conds = HashMap::new();
            let cheap = tree_ops(e) <= IFCONV_WORK;
            let conv = if cheap { ifconv(e, &mut n2, &mut budget, &mut Vec::new(), &mut conds) } else { None };
            let ints_only = |leaves: &Vec<Leaf>| {
                (0..leaves[0].1.len()).all(|j| {
                    leaves.iter().all(|(_, a)| a[j] == leaves[0].1[j]) || leaves.iter().all(|(_, a)| int_expr(&a[j], fid, &tys))
                })
            };
            let (lets, g, leaves) = conv.filter(|(_, g, l)| *g == fid && ints_only(l))?;
            next = n2;
            let args: Vec<Core> = (0..leaves[0].1.len()).map(|j| select_arg(&leaves, j, &conds)).collect();
            let mut out = Core::Call(g, args);
            for (v, r) in lets.into_iter().rev() {
                out = Core::Let(v, Box::new(r), Box::new(out));
            }
            Some(out)
        });
    }
    out
}
