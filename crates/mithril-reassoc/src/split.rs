//! A loop ending in `s = E(s, ..)` whose other work ignores `s` runs that work as a tree
//! of independent calls (parallel), then applies `E` in iteration order: same result.
//!
//! A split costs a leaf and an apply step per iteration, a constant, so it only pays
//! when an iteration's work can grow. The interaction rules decide that: work whose
//! residual, after compile-time reduction, still reaches a call cycle can grow; work
//! the rules reduce to plain operations stays one native loop.

use mithril_front::ast::{Expr, FnDef, Module, Stmt};
use mithril_front::core::Core;
use mithril_front::desugar::{assigned_names, free_reads_expr, free_reads_stmts};
use std::collections::BTreeSet;

const TREE: &str = "
@data
class __TreeK:
    __LeafK: (FIELDS)
    __NodeK: (__l, __r)
    __NoneK: ()

def __genK(__lo, __hi, INV):
    if __hi - __lo == 1:
        return __workK(__lo, INV)
    if __hi - __lo < 1:
        return __NoneK()
    __mid = __lo + (__hi - __lo) // 2
    return __NodeK(__genK(__lo, __mid, INV), __genK(__mid, __hi, INV))

def __appK(__s, __t, __lo, __hi, INV):
    match __t:
        case __LeafK(PAY):
            return __updK(__s, PAY, __lo, INV)
        case __NodeK(__l, __r):
            __mid = __lo + (__hi - __lo) // 2
            return __appK(__appK(__s, __l, __lo, __mid, INV), __r, __mid, __hi, INV)
        case __NoneK():
            return __s
";

pub fn split_independent_loops(m: &mut Module) {
    // a loop in a function reached from parallel work (a split loop's work, a proven
    // fold's body, or a function that calls itself twice on one path) is kept
    // the residual decides which loops may split; reduce only when some loop could
    let every: BTreeSet<String> = m.fns.iter().map(|f| f.name.clone()).collect();
    if run(&mut m.clone(), &BTreeSet::new(), &every).fns.is_empty() {
        return;
    }
    let grows = unbounded(m);
    let mut trial = m.clone();
    let tried = run(&mut trial, &BTreeSet::new(), &grows);
    let forking = m.fns.iter().filter(|f| self_calls(&f.body, &f.name) >= 2).map(|f| f.name.clone());
    let mut inside: BTreeSet<String> = forking.collect();
    let mut todo: Vec<String> = tried.fns.iter().filter(|f| f.name.starts_with("__work")).map(|f| f.name.clone()).collect();
    todo.extend(inside.iter().cloned());
    let mut folded = BTreeSet::new();
    m.fns.iter().for_each(|f| f.body.iter().for_each(|st| fold_calls(st, &mut folded)));
    inside.extend(folded.iter().cloned());
    todo.extend(folded);
    while let Some(name) = todo.pop() {
        let Some(f) = trial.fns.iter().chain(&tried.fns).find(|f| f.name == name) else { continue };
        let mut called = BTreeSet::new();
        f.body.iter().for_each(|st| stmt_calls(st, &mut called));
        todo.extend(called.into_iter().filter(|g| inside.insert(g.clone())));
    }
    let helpers = run(m, &inside, &grows);
    m.datas.extend(helpers.datas);
    m.fns.extend(helpers.fns);
}

fn run(m: &mut Module, keep: &BTreeSet<String>, grows: &BTreeSet<String>) -> Module {
    let mut helpers = Module::default();
    let user: BTreeSet<String> = m.fns.iter().map(|f| f.name.clone()).collect();
    for f in m.fns.iter_mut().filter(|f| !keep.contains(&f.name)) {
        let mut vars: BTreeSet<String> = f.params.iter().cloned().collect();
        vars.extend(assigned_names(&f.body));
        f.body = rewrite(std::mem::take(&mut f.body), &vars, &user, grows, &mut helpers);
    }
    helpers
}

/// Functions whose work can still grow after compile-time reduction. The module is
/// reduced by the interaction rules; a function grows when its residual reaches a call
/// cycle (a loop or recursion the rules could not unfold) or applies a closure.
fn unbounded(m: &Module) -> BTreeSet<String> {
    let all = || m.fns.iter().map(|f| f.name.clone()).collect();
    let Ok(cm) = mithril_front::desugar(m) else { return all() };
    // which residuals reach a call cycle: the per-function pass decides it
    // (clones on known arguments change no call cycle)
    let (sm, _) = mithril_net::reduce::specialize_fns(&cm, mithril_net::REDUCE_FUEL);
    let n = sm.fns.len();
    let mut calls = vec![BTreeSet::new(); n];
    let mut opaque = vec![false; n];
    for (i, f) in sm.fns.iter().enumerate() {
        f.body.walk(&mut |e| match e {
            Core::Call(g, _) => {
                calls[i].insert(*g as usize);
            }
            Core::App(..) => opaque[i] = true,
            _ => {}
        });
    }
    let reach = |from: usize| {
        let (mut seen, mut todo) = (BTreeSet::new(), vec![from]);
        while let Some(g) = todo.pop() {
            for &h in &calls[g] {
                if seen.insert(h) {
                    todo.push(h);
                }
            }
        }
        seen
    };
    let reaches: Vec<BTreeSet<usize>> = (0..n).map(reach).collect();
    let cyclic: Vec<bool> = (0..n).map(|i| reaches[i].contains(&i) || opaque[i]).collect();
    sm.fns
        .iter()
        .enumerate()
        .filter(|(i, _)| cyclic[*i] || reaches[*i].iter().any(|&j| cyclic[j]))
        .map(|(_, f)| f.name.clone())
        .collect()
}

/// The most calls to `name` one activation of `body` can make.
fn self_calls(body: &[Stmt], name: &str) -> usize {
    let count = |e: &Expr| count_calls(e, name);
    body.iter()
        .map(|st| match st {
            Stmt::Assign(_, e) | Stmt::Return(e) | Stmt::ExprStmt(e) => count(e),
            Stmt::If(c, a, b) => count(c) + self_calls(a, name).max(self_calls(b, name)),
            Stmt::While(c, b) | Stmt::For(_, c, b, _) => count(c) + self_calls(b, name),
            Stmt::Match(e, arms) => count(e) + arms.iter().map(|(_, b)| self_calls(b, name)).max().unwrap_or(0),
            Stmt::Break | Stmt::Continue => 0,
        })
        .sum()
}

fn count_calls(e: &Expr, name: &str) -> usize {
    e.fold(&mut |e, counts| {
        let nested = if matches!(e, Expr::IfExp(..)) { counts[0] + counts[1].max(counts[2]) } else { counts.iter().sum() };
        nested + matches!(e, Expr::Call(f, _) if f == name) as usize
    })
}

/// Calls made in the bodies of proven fold loops.
fn fold_calls(st: &Stmt, out: &mut BTreeSet<String>) {
    match st {
        Stmt::For(_, _, body, Some(_)) => body.iter().for_each(|st| stmt_calls(st, out)),
        Stmt::For(_, _, b, None) | Stmt::While(_, b) => b.iter().for_each(|st| fold_calls(st, out)),
        Stmt::If(_, a, b) => a.iter().chain(b).for_each(|st| fold_calls(st, out)),
        Stmt::Match(_, arms) => arms.iter().flat_map(|(_, b)| b).for_each(|st| fold_calls(st, out)),
        _ => {}
    }
}

fn stmt_calls(st: &Stmt, out: &mut BTreeSet<String>) {
    let (head, blocks) = st.parts();
    head.fold(&mut |e, _: Vec<()>| {
        if let Expr::Call(name, _) = e { out.insert(name.clone()); }
    });
    for block in blocks { for st in block { stmt_calls(st, out); } }
}

fn rewrite(stmts: Vec<Stmt>, vars: &BTreeSet<String>, user: &BTreeSet<String>, grows: &BTreeSet<String>, helpers: &mut Module) -> Vec<Stmt> {
    let mut out = Vec::new();
    for (i, s) in stmts.iter().enumerate() {
        let Stmt::For(v, n, body, fold) = s else {
            out.push(s.clone());
            continue;
        };
        let body = rewrite(body.clone(), vars, user, grows, helpers);
        match fold.is_none().then(|| split(v, n, &body, &stmts[i + 1..], vars, user, grows, helpers)).flatten() {
            Some(new) => out.extend(new),
            None => out.push(Stmt::For(v.clone(), n.clone(), body, fold.clone())),
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn split(v: &str, n: &Expr, body: &[Stmt], rest: &[Stmt], vars: &BTreeSet<String>, user: &BTreeSet<String>, grows: &BTreeSet<String>, helpers: &mut Module) -> Option<Vec<Stmt>> {
    let (Stmt::Assign(s, upd), before) = body.split_last()? else { return None };
    let mut work = before.to_vec();
    let mut local = BTreeSet::new();
    for st in &work {
        let Stmt::Assign(x, e) = st else { return None };
        let r = reads(e);
        if x == s || x == v || r.contains(s) || r.iter().any(|y| assigned_names(&work).contains(y) && !local.contains(y)) {
            return None;
        }
        local.insert(x.clone());
    }
    let mut later = BTreeSet::new();
    free_reads_stmts(rest, &mut later);
    if !reads(upd).contains(s) || s == v || local.iter().any(|x| later.contains(x)) || upd.any(&|e| matches!(e, Expr::IfExp(..) | Expr::Bool2(..) | Expr::Not(_))) {
        return None;
    }

    // calls in the update that do not read `s` become work too
    let k = helpers.fns.len();
    let calls = |e: &Expr| e.any(&|x| matches!(x, Expr::Call(f, _) if user.contains(f) || helpers.fns.iter().any(|h| &h.name == f)));
    let upd = hoist(upd, s, &calls, &format!("__w{k}_"), &mut work);
    // only work that can grow pays for a leaf; helpers made by earlier splits recurse
    let grows_call = |e: &Expr| e.any(&|x| matches!(x, Expr::Call(f, _) if grows.contains(f) || helpers.fns.iter().any(|h| &h.name == f)));
    if !work.iter().any(|st| matches!(st, Stmt::Assign(_, e) if grows_call(e))) {
        return None; // bounded work stays a native loop
    }

    let defined = assigned_names(&work);
    let payload: Vec<String> = reads(&upd).into_iter().filter(|x| defined.contains(x)).collect();
    let mut free = reads(&upd);
    free_reads_stmts(&work, &mut free);
    let inv: Vec<String> = free.into_iter().filter(|x| vars.contains(x) && x != s && x != v && !defined.contains(x)).collect();

    if payload.is_empty() {
        return None;
    }
    // a leaf holds the payload values as separate fields, so the apply reads plain values
    let leaf = Expr::Call(format!("__Leaf{k}"), payload.iter().map(|x| Expr::Var(x.clone())).collect());
    work.push(Stmt::Return(leaf));
    let params = |head: &[&str]| head.iter().map(|x| x.to_string()).chain(inv.iter().cloned()).collect::<Vec<_>>();
    helpers.fns.push(FnDef { name: format!("__work{k}"), params: params(&[v]), body: work });
    let mut upd_params = vec![s.clone()];
    upd_params.extend(payload.iter().cloned());
    upd_params.push(v.to_string());
    upd_params.extend(inv.iter().cloned());
    helpers.fns.push(FnDef { name: format!("__upd{k}"), params: upd_params, body: vec![Stmt::Return(upd)] });
    let fields = if payload.len() == 1 { format!("{},", payload[0]) } else { payload.join(", ") };
    let src = TREE.replace('K', &k.to_string()).replace("FIELDS", &fields).replace("PAY", &payload.join(", "));
    let src = src.replace("INV", &inv.join(", ")).replace(", )", ")");
    let tree = mithril_front::parse(&src).expect("loop split helpers parse");
    helpers.datas.extend(tree.datas);
    helpers.fns.extend(tree.fns);

    let call = |f: String, mut args: Vec<Expr>| {
        args.extend(inv.iter().map(|x| Expr::Var(x.clone())));
        Expr::Call(f, args)
    };
    let (nn, tt) = (format!("__n{k}"), format!("__t{k}"));
    let var = |x: &str| Expr::Var(x.into());
    Some(vec![
        Stmt::Assign(nn.clone(), n.clone()),
        Stmt::Assign(tt.clone(), call(format!("__gen{k}"), vec![Expr::Int(0), var(&nn)])),
        Stmt::Assign(s.clone(), call(format!("__app{k}"), vec![var(s), var(&tt), Expr::Int(0), var(&nn)])),
    ])
}

/// Moves each largest part of `e` that ignores `s` and calls a user function into `work`.
fn hoist(e: &Expr, s: &str, calls: &dyn Fn(&Expr) -> bool, prefix: &str, work: &mut Vec<Stmt>) -> Expr {
    let mut h = |x: &Expr| Box::new(hoist(x, s, calls, prefix, work));
    match e {
        _ if !reads(e).contains(s) && calls(e) => {
            let x = format!("{prefix}{}", work.len());
            work.push(Stmt::Assign(x.clone(), e.clone()));
            Expr::Var(x)
        }
        Expr::Bin(op, a, b) => Expr::Bin(*op, h(a), h(b)),
        Expr::Cmp(op, a, b) => Expr::Cmp(*op, h(a), h(b)),
        Expr::Index(a, b) => Expr::Index(h(a), h(b)),
        Expr::Neg(a) => Expr::Neg(h(a)),
        Expr::Call(f, xs) => Expr::Call(f.clone(), xs.iter().map(|x| *h(x)).collect()),
        Expr::Tuple(xs) => Expr::Tuple(xs.iter().map(|x| *h(x)).collect()),
        _ => e.clone(),
    }
}

fn reads(e: &Expr) -> BTreeSet<String> {
    let mut r = BTreeSet::new();
    free_reads_expr(e, &mut r);
    r
}
