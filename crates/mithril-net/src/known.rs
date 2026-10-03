//! Specialization on known arguments.
//!
//! The per-function pass (`reduce::specialize_fns`) reduces each body over
//! unknown parameters, so a constant reaching a function through a call
//! stays a parameter inside it: heat2d's `w = 1024` reaches its stencil only
//! as a runtime value, and `i % w` stays a division. Here a call that
//! passes a constant to a parameter the callee never changes names a clone
//! of the callee in which that parameter is the constant; the call is
//! retargeted and the module specialized again, so the rules fold the
//! constant through the clone (as unfolding with that argument would). The
//! clone is the memoized residual of the callee under that argument: a
//! recursive callee's own calls pass the parameter unchanged, so after its
//! specialization they pass the constant and are retargeted to the same
//! clone.
//!
//! A parameter can be fixed when the callee is on no call cycle, or calls
//! only itself and passes that parameter through unchanged in every such
//! call. A proven fold keeps its counter, bound and accumulator (its split
//! relies on that shape). Only int constants are fixed, and the number of
//! clones is bounded. A parameter is fixed only where the constant changes
//! the cost of repeated work: it is an unchanged parameter of a loop (a
//! function that calls itself) that the loop divides by (or shifts by), or
//! the function passes it unchanged to such a parameter of a callee. A constant used once (a sphere's radius, a wall's
//! position) gains nothing from a clone and costs a specialization: the
//! Whitted demo's constant arguments cloned every shape function and its
//! code generation went from seconds to minutes. A loop's own test is not
//! fixed either (a known bound would unroll the loop and evaluate its body
//! at compile time in each clone).

use crate::reduce::{specialize_fns, specialize_some, SpecReport};
use mithril_front::core::{Core, CoreFn, CoreModule};
use std::collections::{BTreeMap, HashMap};

/// Clones a module may gain (each is a specialized copy of a function).
const MAX_CLONES: usize = 64;
/// Rounds of clone and re-specialize (a chain of calls fixes one level per
/// round).
const MAX_ROUNDS: usize = 8;

/// Specialize every function by the interaction rules (see
/// `reduce::specialize_fns`), and specialize callees on the constants their
/// calls pass to parameters they never change.
pub fn specialize(m: &CoreModule, fuel: u64) -> (CoreModule, Vec<SpecReport>) {
    let (mut out, mut reports) = specialize_fns(m, fuel);
    // (original callee, fixed (parameter, value)s) -> clone
    let mut clones: HashMap<(u32, Vec<(usize, i64)>), u32> = HashMap::new();
    // clone -> (original, fixed): a clone's fixed parameters stay fixed, and
    // fixing more of them names a clone of the original under the union
    let mut origin: HashMap<u32, (u32, Vec<(usize, i64)>)> = HashMap::new();
    for _ in 0..MAX_ROUNDS {
        let fixable = repeated(&out, (0..out.fns.len()).map(|g| fixable_params(&out, g as u32)).collect());
        let mut changed = false;
        let mut todo = vec![false; out.fns.len()];
        for f in 0..out.fns.len() {
            let body = std::mem::replace(&mut out.fns[f].body, Core::Num(0));
            let mut hit = false;
            let body = retarget(body, &mut |g, args| {
                let (g0, mut fixed) = origin.get(&g).cloned().unwrap_or((g, Vec::new()));
                let before = fixed.len();
                for (i, a) in args.iter().enumerate() {
                    if let Core::Num(n) = a {
                        if fixable.get(g as usize).is_some_and(|fx| fx[i]) && !fixed.iter().any(|(j, _)| *j == i) {
                            fixed.push((i, *n));
                        }
                    }
                }
                if fixed.len() == before {
                    return None;
                }
                fixed.sort();
                if let Some(c) = clones.get(&(g0, fixed.clone())) {
                    hit |= *c != g;
                    return (*c != g).then_some(*c);
                }
                if clones.len() >= MAX_CLONES {
                    return None;
                }
                let c = out_len_hint(&clones, m.fns.len());
                clones.insert((g0, fixed.clone()), c);
                origin.insert(c, (g0, fixed));
                changed = true;
                hit = true;
                Some(c)
            });
            out.fns[f].body = body;
            todo[f] = hit;
        }
        // materialize the clones named this round, in id order
        let mut fresh: BTreeMap<u32, (u32, Vec<(usize, i64)>)> = BTreeMap::new();
        for ((g, fixed), c) in &clones {
            if *c as usize >= out.fns.len() {
                fresh.insert(*c, (*g, fixed.clone()));
            }
        }
        for (c, (g, fixed)) in fresh {
            assert_eq!(c as usize, out.fns.len(), "ICE: clone ids are dense");
            // from the original's specialized body; its calls to itself
            // become calls to the clone
            out.fns.push(clone_with(&out.fns[g as usize], g, c, &fixed));
            todo.push(true);
        }
        if !changed && !todo.contains(&true) {
            break;
        }
        // only clones and functions whose calls were retargeted change
        let (o, r) = specialize_some(&out, fuel, &todo);
        out = o;
        for rep in r {
            match reports.iter_mut().find(|x| x.name == rep.name) {
                Some(x) => *x = rep,
                None => reports.push(rep),
            }
        }
    }
    // a function no call reaches any more (its callers now call a clone)
    // keeps its id but not its code
    if !clones.is_empty() {
        let live = reachable(&out);
        for (f, fun) in out.fns.iter_mut().enumerate() {
            if !live[f] {
                fun.body = Core::Num(0);
                fun.fold = None;
                fun.self_tail_rec = false;
            }
        }
    }
    (out, reports)
}

/// Functions a call can reach from `main`.
fn reachable(m: &CoreModule) -> Vec<bool> {
    let mut live = vec![false; m.fns.len()];
    let mut stack = vec![m.main];
    while let Some(f) = stack.pop() {
        if std::mem::replace(&mut live[f as usize], true) {
            continue;
        }
        m.fns[f as usize].body.walk(&mut |e| {
            if let Core::Call(h, _) = e {
                stack.push(*h);
            }
        });
    }
    live
}

/// The next clone's function id (clones are numbered after the module's
/// functions, in the order they are named).
fn out_len_hint(clones: &HashMap<(u32, Vec<(usize, i64)>), u32>, base: usize) -> u32 {
    (base + clones.len()) as u32
}

/// Function `g` (id `gid`) as clone `cid`, its parameters `fixed` replaced
/// by their constants. Its calls to itself become calls to the clone that
/// still pass the fixed parameters through (they hold the constants), so a
/// fold keeps the shape its split relies on.
fn clone_with(g: &CoreFn, gid: u32, cid: u32, fixed: &[(usize, i64)]) -> CoreFn {
    let subst: HashMap<u32, i64> = fixed.iter().map(|(i, n)| (*i as u32, *n)).collect();
    fn go(e: Core, subst: &HashMap<u32, i64>, gid: u32, cid: u32) -> Core {
        match e {
            Core::Var(v) if subst.contains_key(&v) => Core::Num(subst[&v]),
            Core::Call(h, args) if h == gid => Core::Call(
                cid,
                args.into_iter().enumerate().map(|(i, a)| if a == Core::Var(i as u32) && subst.contains_key(&(i as u32)) { a } else { go(a, subst, gid, cid) }).collect(),
            ),
            e => crate::expose::map_kids(e, &mut |k| go(k, subst, gid, cid)),
        }
    }
    let tag: String = fixed.iter().map(|(i, n)| format!("_{i}k{n}")).collect();
    CoreFn { name: format!("{}{tag}", g.name), arity: g.arity, body: go(g.body.clone(), &subst, gid, cid), self_tail_rec: g.self_tail_rec, fold: g.fold.clone() }
}

/// Rewrite every call `g(args)` to `h(args)` where `pick(g, args)` names `h`.
fn retarget(e: Core, pick: &mut dyn FnMut(u32, &[Core]) -> Option<u32>) -> Core {
    let e = crate::expose::map_kids(e, &mut |k| retarget(k, pick));
    match e {
        Core::Call(g, args) => match pick(g, &args) {
            Some(h) => Core::Call(h, args),
            None => Core::Call(g, args),
        },
        e => e,
    }
}

/// Of the `fixable` parameters, those whose constant reaches repeated work:
/// an unchanged parameter of a loop, or one passed unchanged to such a
/// parameter of a callee (a fixpoint over the call graph).
fn repeated(m: &CoreModule, fixable: Vec<Vec<bool>>) -> Vec<Vec<bool>> {
    let looping: Vec<bool> = (0..m.fns.len()).map(|g| m.fns[g].body.any(&mut |e| matches!(e, Core::Call(h, _) if *h as usize == g).then_some(true))).collect();
    // a loop's own test (the branch that decides whether it calls itself
    // again) stays: a known bound with a known start lets the per-function
    // pass unroll the loop and evaluate its body at compile time in every
    // clone (the Whitted demo's pixel loops: 1,180 traced pixels per clone)
    let control = |g: usize| -> Vec<bool> {
        let mut c = vec![false; fixable[g].len()];
        if let Core::If(test, ..) = &m.fns[g].body {
            test.walk(&mut |e| {
                if let Core::Var(v) = e {
                    if (*v as usize) < c.len() {
                        c[*v as usize] = true;
                    }
                }
            });
        }
        c
    };
    // a constant divisor or shift count becomes a shift, mask or multiply;
    // other uses of a constant measured no gain (design.md)
    let divisor = |g: usize| -> Vec<bool> {
        use mithril_front::ast::BinOp::*;
        let mut d = vec![false; fixable[g].len()];
        m.fns[g].body.walk(&mut |e| {
            if let Core::Op2(Div | FloorDiv | Mod | Shl | Shr, _, b) = e {
                if let Core::Var(v) = **b {
                    if (v as usize) < d.len() {
                        d[v as usize] = true;
                    }
                }
            }
        });
        d
    };
    let mut rep: Vec<Vec<bool>> = fixable.iter().enumerate().map(|(g, fx)| {
        let (c, d) = (control(g), divisor(g));
        fx.iter().enumerate().map(|(i, &f)| f && looping[g] && !c[i] && d[i]).collect()
    }).collect();
    loop {
        let mut changed = false;
        for g in 0..m.fns.len() {
            let mut reach = vec![false; fixable[g].len()];
            m.fns[g].body.walk(&mut |e| {
                if let Core::Call(h, args) = e {
                    for (j, a) in args.iter().enumerate() {
                        if let Core::Var(i) = a {
                            if (*i as usize) < reach.len() && rep.get(*h as usize).is_some_and(|r| r.get(j).copied().unwrap_or(false)) {
                                reach[*i as usize] = true;
                            }
                        }
                    }
                }
            });
            for (i, r) in reach.into_iter().enumerate() {
                if r && fixable[g][i] && !rep[g][i] {
                    rep[g][i] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            return rep;
        }
    }
}

/// Which parameters of `g` a caller's constant may fix (see the module doc).
fn fixable_params(m: &CoreModule, g: u32) -> Vec<bool> {
    let f = &m.fns[g as usize];
    if g == m.main {
        return vec![false; f.arity];
    }
    let mut ok = vec![true; f.arity];
    // calls to itself pass the parameter unchanged; a call to anything that
    // reaches back to `g` ends it
    let mut mutual = false;
    f.body.walk(&mut |e| {
        if let Core::Call(h, args) = e {
            if *h == g {
                for (i, a) in args.iter().enumerate() {
                    if *a != Core::Var(i as u32) {
                        ok[i] = false;
                    }
                }
            } else if reaches(m, *h, g) {
                mutual = true;
            }
        }
    });
    if mutual {
        return vec![false; f.arity];
    }
    if let Some(acc) = f.fold.as_ref().map(|_| fold_acc(f)) {
        // a proven fold keeps its counter, bound and accumulator
        for i in [Some(0), Some(1), acc].into_iter().flatten() {
            if i < ok.len() {
                ok[i] = false;
            }
        }
    }
    ok
}

/// The accumulator of a fold helper `If(cond, then, Var acc)`.
fn fold_acc(f: &CoreFn) -> Option<usize> {
    match &f.body {
        Core::If(_, _, e) => match e.as_ref() {
            Core::Var(k) => Some(*k as usize),
            _ => None,
        },
        _ => None,
    }
}

/// Whether a call to `from` can reach a call to `to`.
fn reaches(m: &CoreModule, from: u32, to: u32) -> bool {
    let mut seen = vec![false; m.fns.len()];
    let mut stack = vec![from];
    while let Some(f) = stack.pop() {
        if f == to {
            return true;
        }
        if std::mem::replace(&mut seen[f as usize], true) {
            continue;
        }
        m.fns[f as usize].body.walk(&mut |e| {
            if let Core::Call(h, _) = e {
                stack.push(*h);
            }
        });
    }
    false
}
