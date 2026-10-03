//! Value-only entries for callers that discard the completed task's budget.
//! Originals keep every charge. Unknown effects and budget observers reject
//! an entire call closure; only proven dead accounting is erased in clones.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn charge<'a>(s: &'a S, fuel: &str) -> Option<&'a E> {
    match s {
        S::Do(E::Call { f, ctx: false, args }) if matches!(f.as_str(), "work_fuel" | "native_work_fuel") && args.len() == 2 && args[0] == v(fuel) => Some(&args[1]),
        _ => None,
    }
}
fn accounting(s: &S, counters: &BTreeSet<String>) -> bool {
    matches!(s, S::Let(Pat::One(n), _, _) | S::Set(n, _) if counters.contains(n))
}
fn frame_helper(f: &str) -> bool {
    operations::helper(f).is_some_and(|h| h.effect == operations::Effect::Frame)
}
fn helper(f: &str) -> bool {
    frame_helper(f) || matches!(f, "as_i" | "num" | "floor_div" | "py_mod" | "stack_guard" | "native_frames" | "native_records" | "native_ok" | "native_check")
}
fn check(f: &FnDef, functions: &BTreeMap<&str, &FnDef>) -> Option<(String, BTreeSet<String>, BTreeSet<String>)> {
    if f.ctx || !matches!(f.ret, Ty::I64 | Ty::Tup(_)) { return None; }
    let (fuel, Ty::RefI64) = f.params.first()? else { return None };
    if f.params[1..].iter().any(|(_, t)| !matches!(t, Ty::I64 | Ty::U64 | Ty::U32 | Ty::U16 | Ty::U8 | Ty::Bool)) { return None; }
    let mut counters = BTreeSet::new();
    let mut frames = BTreeSet::new();
    walk_stmts(&f.body, &mut |s| {
        if let Some(E::V(n)) = charge(s, fuel) { counters.insert(n.clone()); }
        if let S::Let(Pat::One(n), Ty::Frames | Ty::FrameRecords(_), _) = s { frames.insert(n.clone()); }
    });
    let mut valid = true;
    let mut initialized = BTreeSet::new();
    let mut calls = BTreeSet::new();
    let total = |e: &E, self_read: bool| e.all(&|e| match e {
        E::Call { .. } | E::Neg(_) | E::Bin(Bop::Div, ..) | E::Deref(_) | E::Ref(_) | E::Addr(_) | E::Flo(_) | E::Const(_)
            | E::Idx(..) | E::Arr(_) | E::Tup(_) | E::Slice(_) | E::Ok(_) | E::Err(_) => false,
        E::V(n) => n != fuel && (self_read || !counters.contains(n)),
        _ => true,
    });
    walk_stmts(&f.body, &mut |s| {
        if let Some(amount) = charge(s, fuel) { valid &= total(amount, true); return; }
        if accounting(s, &counters) {
            valid &= match s {
                S::Let(Pat::One(n), Ty::I64, E::Int(0 | 1, Ty::I64)) => initialized.insert(n.clone()),
                S::Set(n, e) => {
                    let mut base = e;
                    while let E::Bin(Bop::Add, a, b) = base {
                        valid &= total(b, false);
                        base = a;
                    }
                    *base == v(n)
                },
                _ => false,
            };
            return;
        }
        if matches!(s, S::Store(..) | S::Fn(_) | S::Try(..) | S::Res(..))
            || matches!(s, S::Set(n, _) | S::Decl(n, _) if n == fuel)
            || matches!(s, S::Let(p, _, _) if pat_names(p).iter().any(|n| n == fuel)) { valid = false; }
        let Some(e) = s.parts().0 else { return };
        let mut e = e.clone();
        e.rewrite(&mut |e| if let E::Call { f, ctx, args } = e {
            if let Some(callee) = functions.get(f.as_str()) {
                valid &= !*ctx && args.len() == callee.params.len() && args.first() == Some(&v(fuel)) && matches!(callee.params.first(), Some((_, Ty::RefI64)));
                calls.insert(f.clone());
                if !args.is_empty() { args.remove(0); }
            } else {
                valid &= !*ctx && helper(f);
                if frame_helper(f) {
                    valid &= matches!(args.first(), Some(E::Ref(n) | E::Addr(n)) if frames.contains(n));
                    if !args.is_empty() { args[0] = u64_(0); }
                }
            }
        });
        valid &= e.all(&|e| match e {
            E::V(n) => n != fuel && !counters.contains(n) && !frames.contains(n),
            E::Deref(_) | E::Ref(_) | E::Addr(_) | E::Flo(_) | E::Const(_) | E::Slice(_) => false,
            _ => true,
        });
    });
    valid &= initialized == counters;
    valid.then(|| (fuel.clone(), counters, calls))
}
/// Clone checked native closures for completed-task callers. Returned root
/// names have no budget parameter; callers must not need a residual budget.
pub fn entries(fns: &[FnDef], roots: &[String]) -> (BTreeMap<String, String>, Vec<FnDef>) {
    let functions: BTreeMap<_, _> = fns.iter().map(|f| (f.name.as_str(), f)).collect();
    let checked: BTreeMap<_, _> = fns.iter().map(|f| (f.name.as_str(), check(f, &functions))).collect();
    let mut selected = BTreeSet::new();
    let mut entries = BTreeMap::new();
    for root in roots {
        let mut seen = BTreeSet::new();
        let mut todo = vec![root.clone()];
        let mut valid = true;
        while let Some(name) = todo.pop() {
            if !seen.insert(name.clone()) { continue; }
            match checked.get(name.as_str()).and_then(Option::as_ref) {
                Some((_, _, calls)) => todo.extend(calls.iter().cloned()),
                None => { valid = false; break; }
            }
        }
        if valid && seen.iter().all(|n| !functions.contains_key(format!("value_{n}").as_str())) {
            entries.insert(root.clone(), format!("value_{root}")); selected.extend(seen);
        }
    }
    let mut clones = Vec::new();
    for name in &selected {
        let (fuel, counters, _) = checked[name.as_str()].as_ref().unwrap();
        let mut f = (*functions[name.as_str()]).clone();
        f.name = format!("value_{name}"); f.params.remove(0);
        retain_rewrite(&mut f.body, &mut |s| charge(s, fuel).is_none() && !accounting(s, counters), &mut |e| {
            if let E::Call { f, args, .. } = e {
                if selected.contains(f) { *f = format!("value_{f}"); args.remove(0); }
            }
        });
        clones.push(f);
    }
    (entries, clones)
}
