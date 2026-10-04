//! Defunctionalize native call cycles. Only return addresses and live locals
//! cross a call; arithmetic, ownership and work charges remain the original LIR.
use crate::lir::*;
use std::collections::{BTreeMap, BTreeSet};

type Env = BTreeMap<String, E>;
#[derive(Clone)]
enum End {
    Jump(usize),
    Branch(E, usize, usize),
    Switch(E, Vec<(u64, usize)>, Option<usize>),
    Call(String, Vec<E>, Vec<String>, usize),
    Return(Vec<E>),
}
impl End {
    fn targets(&self) -> Vec<usize> {
        match self {
            Self::Jump(j) | Self::Call(_, _, _, j) => vec![*j],
            Self::Branch(_, a, b) => vec![*a, *b],
            Self::Switch(_, arms, d) => arms.iter().map(|(_, j)| *j).chain(d.iter().copied()).collect(),
            Self::Return(_) => vec![],
        }
    }
    fn exprs(&self) -> &[E] {
        match self {
            Self::Branch(e, ..) | Self::Switch(e, ..) => std::slice::from_ref(e),
            Self::Call(_, args, ..) | Self::Return(args) => args,
            Self::Jump(_) => &[],
        }
    }

    fn control(&self) -> Option<S> {
        Some(match self {
            Self::Jump(j) => S::Jump(u64_(*j as u64)),
            Self::Branch(e, a, b) => S::If(e.clone(), jump(*a), jump(*b)),
            Self::Switch(e, arms, d) => S::Switch(e.clone(), arms.iter().map(|(k, j)| (*k, jump(*j))).collect(), d.map(jump)),
            Self::Call(..) | Self::Return(_) => return None,
        })
    }
}
struct Block { body: Vec<S>, end: End }
struct Machine<'a> {
    funcs: &'a BTreeMap<String, FnDef>,
    blocks: Vec<Block>,
    locals: BTreeMap<String, Ty>,
    params: BTreeMap<String, Vec<String>>,
    entries: BTreeMap<String, usize>,
    active: Vec<String>,
    loops: BTreeSet<usize>,
    bounds: &'a BTreeMap<String, BTreeSet<String>>,
    narrow: BTreeSet<String>,
}
// Both summary certification and execution lower the same call components.
fn calls(body: &[S], out: &mut BTreeSet<String>) {
    walk_exprs(body, &mut |e| if let E::Call { f, .. } = e { if f.starts_with("s_") { out.insert(f.clone()); } });
}
fn closure(edges: &mut BTreeMap<String, BTreeSet<String>>) {
    loop {
        let old = edges.clone();
        for next in edges.values_mut() { for name in next.clone() { next.extend(old.get(&name).into_iter().flatten().cloned()); } }
        if *edges == old { break; }
    }
}
fn components(fns: &[FnDef]) -> Vec<(String, BTreeMap<String, FnDef>)> {
    let native: BTreeMap<_, _> = fns.iter().filter(|f| f.name.starts_with("s_")).map(|f| (f.name.clone(), f.clone())).collect();
    let mut reach: BTreeMap<_, _> = native.iter().map(|(name, f)| {
        let mut next = BTreeSet::new(); calls(&f.body, &mut next); (name.clone(), next)
    }).collect();
    closure(&mut reach);
    let mut seen = BTreeSet::new();
    let mut groups = Vec::new();
    for name in native.keys() {
        if seen.contains(name) || !reach[name].contains(name) { continue; }
        let group: BTreeMap<_, _> = native.iter().filter(|(n, _)| reach[name].contains(*n) && reach[*n].contains(name)).map(|(n, f)| (n.clone(), f.clone())).collect();
        seen.extend(group.keys().cloned());
        groups.push((name.clone(), group));
    }
    groups
}
fn single_entry(group: &BTreeMap<String, FnDef>, entry: &str) -> bool {
    let mut reach: BTreeMap<_, _> = group.iter().filter(|(n, _)| *n != entry).map(|(n, f)| {
        let mut next = BTreeSet::new(); calls(&f.body, &mut next);
        next.retain(|n| group.contains_key(n) && n != entry);
        (n.clone(), next)
    }).collect();
    closure(&mut reach);
    reach.iter().all(|(n, next)| !next.contains(n))
}
impl<'a> Machine<'a> {
    fn new(funcs: &'a BTreeMap<String, FnDef>, bounds: &'a BTreeMap<String, BTreeSet<String>>, entry: Option<&str>) -> Self {
        let mut m = Self { funcs, blocks: vec![], locals: BTreeMap::new(), params: BTreeMap::new(), entries: BTreeMap::new(), active: vec![], loops: BTreeSet::new(), bounds, narrow: BTreeSet::new() };
        for (n, f) in funcs.iter().filter(|(n, _)| entry.is_none_or(|e| *n == e)) {
            let at = m.block();
            m.entries.insert(n.clone(), at);
            let ps = f.params.iter().skip(1).map(|(name, t)| {
                let x = m.local(*t);
                if bounds[n].contains(name) { m.narrow.insert(x.clone()); }
                x
            }).collect();
            m.params.insert(n.clone(), ps);
        }
        for (n, f) in funcs.iter().filter(|(n, _)| entry.is_none_or(|e| *n == e)) {
            let env = f.params.iter().skip(1).map(|(p, _)| p.clone()).zip(m.params[n].iter().map(v)).collect();
            m.active.push(n.clone());
            m.body(&f.body, env, m.entries[n], None, None);
            m.active.pop();
        }
        m
    }
    fn block(&mut self) -> usize {
        let id = self.blocks.len();
        self.blocks.push(Block { body: vec![], end: End::Return(vec![]) });
        id
    }
    fn local(&mut self, ty: Ty) -> String {
        let x = format!("n{}", self.locals.len());
        self.locals.insert(x.clone(), ty);
        x
    }
    fn expr(&mut self, e: &E, env: &Env, at: &mut usize) -> E {
        let mut e = e.clone();
        e.rewrite(&mut |e| {
            if let E::V(x) | E::Deref(x) | E::Ref(x) | E::Addr(x) = e {
                if let Some(n) = env.get(x) {
                    if matches!(e, E::V(_)) { *e = n.clone(); return; }
                    if let E::V(n) = n { match e { E::Deref(x) | E::Ref(x) | E::Addr(x) => *x = n.clone(), _ => {} } }
                }
            }
            if let E::Call { f, args, .. } = &*e {
                if let Some(d) = self.funcs.get(f) {
                    let ret = d.ret;
                    let out: Vec<_> = (0..width(ret)).map(|_| self.local(Ty::I64)).collect();
                    let next = self.block();
                    if self.entries.contains_key(f) {
                        self.blocks[*at].end = End::Call(f.clone(), args[1..].to_vec(), out.clone(), next);
                    } else {
                        let d = d.clone();
                        let mut env = Env::new();
                        for ((name, ty), arg) in d.params.iter().skip(1).zip(&args[1..]) {
                            let n = self.local(*ty);
                            if self.bounds.get(f).is_some_and(|ps| ps.contains(name)) { self.narrow.insert(n.clone()); }
                            self.blocks[*at].body.push(set(&n, arg.clone()));
                            env.insert(name.clone(), v(n));
                        }
                        self.active.push(f.clone());
                        self.body(&d.body, env, *at, None, Some((out.clone(), next)));
                        self.active.pop();
                    }
                    *at = next;
                    *e = if ret == Ty::I64 { v(&out[0]) } else { E::Tup(out.iter().map(v).collect()) };
                }
            }
        });
        e
    }
    fn body(&mut self, body: &[S], mut env: Env, mut at: usize, loop_to: Option<usize>, return_to: Option<(Vec<String>, usize)>) -> Option<usize> {
        for s in body {
            match s {
                S::Let(pat, ty, e) => {
                    let e = self.expr(e, &env, &mut at);
                    if let (Pat::One(x), E::Tup(es)) = (pat, &e) {
                        let vars: Vec<_> = es.iter().map(|_| self.local(Ty::I64)).collect();
                        self.blocks[at].body.extend(vars.iter().zip(es).map(|(n, e)| set(n, e.clone())));
                        env.insert(x.clone(), E::Tup(vars.iter().map(v).collect()));
                        continue;
                    }
                    let ty = if *ty == Ty::Infer { match &e { E::V(n) => self.locals[n], _ => Ty::I64 } } else { *ty };
                    let names = pat_names(pat);
                    let types = match pat { Pat::One(_) => vec![ty], Pat::Arr(_) => vec![Ty::U64; names.len()], Pat::Tup(_) => {
                        let t = if matches!(&e, E::Call { f, .. } if !f.starts_with("s_")) { Ty::U64 } else { Ty::I64 };
                        vec![t; names.len()]
                    }};
                    let vars: Vec<_> = types.into_iter().map(|t| self.local(t)).collect();
                    for (old, n) in names.iter().zip(&vars) { if self.bounds.get(self.active.last().unwrap()).is_some_and(|ps| ps.contains(old)) { self.narrow.insert(n.clone()); } }
                    match (pat, e) {
                        (Pat::One(_), e) => self.blocks[at].body.push(set(&vars[0], e)),
                        (_, E::Tup(es)) => self.blocks[at].body.extend(vars.iter().zip(es).map(|(n, e)| set(n, e))),
                        (pat, e) => {
                            let temps: Vec<_> = vars.iter().map(|n| format!("t_{n}")).collect();
                            let p = if matches!(pat, Pat::Arr(_)) { Pat::Arr(temps.clone()) } else { Pat::Tup(temps.clone()) };
                            self.blocks[at].body.push(S::Let(p, Ty::Infer, e));
                            self.blocks[at].body.extend(vars.iter().zip(temps).map(|(n, t)| set(n, v(t))));
                        }
                    }
                    env.extend(names.into_iter().zip(vars.into_iter().map(v)));
                }
                S::Decl(x, ty) => { let n = self.local(*ty); env.insert(x.clone(), v(n)); }
                S::Set(x, e) | S::Store(x, e) => {
                    let e = self.expr(e, &env, &mut at);
                    let n = match env.get(x) { Some(E::V(n)) => n.clone(), None => x.clone(), _ => unreachable!() };
                    self.blocks[at].body.push(if matches!(s, S::Store(..)) { S::Store(n, e) } else { set(n, e) });
                }
                S::Do(E::Call { f, .. }) if f == "stack_guard" => {}
                S::Do(e) => { let e = self.expr(e, &env, &mut at); self.blocks[at].body.push(do_(e)); }
                S::If(e, a, b) => {
                    let e = self.expr(e, &env, &mut at);
                    let (x, y, join) = (self.block(), self.block(), self.block());
                    self.blocks[at].end = End::Branch(e, x, y);
                    let ends = [self.body(a, env.clone(), x, loop_to, return_to.clone()), self.body(b, env.clone(), y, loop_to, return_to.clone())];
                    for end in ends.iter().flatten() { self.blocks[*end].end = End::Jump(join); }
                    if ends.iter().all(Option::is_none) { return None; }
                    at = join;
                }
                S::Switch(e, arms, default) => {
                    let e = self.expr(e, &env, &mut at);
                    let join = self.block();
                    let mut targets = Vec::new();
                    for (key, body) in arms {
                        let target = self.block();
                        if let Some(end) = self.body(body, env.clone(), target, loop_to, return_to.clone()) { self.blocks[end].end = End::Jump(join); }
                        targets.push((*key, target));
                    }
                    let default = default.as_ref().map(|body| {
                        let target = self.block();
                        if let Some(end) = self.body(body, env.clone(), target, loop_to, return_to.clone()) { self.blocks[end].end = End::Jump(join); }
                        target
                    });
                    self.blocks[at].end = End::Switch(e, targets, default);
                    at = join;
                }
                S::Unreachable => { self.blocks[at].body.push(S::Unreachable); return None; }
                S::Loop(b) => {
                    let head = self.block();
                    self.loops.insert(head);
                    self.blocks[at].end = End::Jump(head);
                    if let Some(end) = self.body(b, env, head, Some(head), return_to.clone()) { self.blocks[end].end = End::Jump(head); }
                    return None;
                }
                S::Continue => { self.blocks[at].end = End::Jump(loop_to.unwrap()); return None; }
                S::Ret(e) => {
                    let e = self.expr(e, &env, &mut at);
                    let es = match e { E::Tup(es) => es, e => vec![e] };
                    if let Some((outs, next)) = &return_to {
                        self.blocks[at].body.extend(outs.iter().zip(es).map(|(n, e)| set(n, e)));
                        self.blocks[at].end = End::Jump(*next);
                    } else {
                        self.blocks[at].end = End::Return(es);
                    }
                    return None;
                }
                S::Comment(_) => {}
                _ => unreachable!("unsupported native statement: {s:?}"),
            }
        }
        Some(at)
    }
    /// A call whose continuation only copies its results to a return is a
    /// tail call: the callee's parameters take the arguments and control
    /// jumps to its entry, reusing the frame. Recursion through tail calls
    /// (a loop written as a function, or a join point calling back into its
    /// function) then pushes no frames.
    fn tail_calls(&mut self) {
        for i in 0..self.blocks.len() {
            let End::Call(f, args, outs, next) = self.blocks[i].end.clone() else { continue };
            if !self.forwards(next, &outs) {
                continue;
            }
            // the arguments may read the callee's parameters: evaluate them all first
            let params = self.params[&f].clone();
            let temps: Vec<String> = params.iter().map(|p| self.local(self.locals[p])).collect();
            for (t, a) in temps.iter().zip(&args) {
                self.blocks[i].body.push(set(t, a.clone()));
            }
            for (p, t) in params.iter().zip(&temps) {
                self.blocks[i].body.push(set(p, v(t)));
            }
            self.blocks[i].end = End::Jump(self.entries[&f]);
        }
    }
    /// From block `at`, control reaches a return of exactly `outs` (in order)
    /// through copies and jumps only.
    fn forwards(&self, mut at: usize, outs: &[String]) -> bool {
        // holds[k]: the names holding result k so far
        let mut holds: Vec<BTreeSet<String>> = outs.iter().map(|o| BTreeSet::from([o.clone()])).collect();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(at) {
                return false;
            }
            let b = &self.blocks[at];
            for s in &b.body {
                let S::Set(n, E::V(m)) = s else { return false };
                for h in holds.iter_mut() {
                    h.remove(n);
                }
                if let Some(k) = holds.iter().position(|h| h.contains(m)) {
                    holds[k].insert(n.clone());
                }
            }
            match &b.end {
                End::Jump(j) => at = *j,
                End::Return(es) => {
                    return es.len() == outs.len() && es.iter().zip(&holds).all(|(e, h)| matches!(e, E::V(n) if h.contains(n)));
                }
                _ => return false,
            }
        }
    }
    fn hoist_captures(&mut self) {
        for i in 0..self.blocks.len() {
            let End::Call(_, args, outs, next) = self.blocks[i].end.clone() else { continue };
            let mut blocked: BTreeSet<_> = outs.into_iter().collect();
            let mut arguments = BTreeSet::new();
            for e in args { e.walk(&mut |e| if let E::V(n) = e { arguments.insert(n.clone()); }); }
            let mut kept = Vec::new();
            let mut prior_reads = BTreeSet::new();
            for s in std::mem::take(&mut self.blocks[next].body) {
                let movable = if let S::Set(n, e) = &s {
                    let mut safe = !arguments.contains(n) && !blocked.contains(n) && !prior_reads.contains(n);
                    e.walk(&mut |e| match e {
                        E::V(n) if blocked.contains(n) => safe = false,
                        E::Call { f, .. } if f != "wrap56" => safe = false,
                        E::Deref(_) | E::Ref(_) | E::Addr(_) | E::Bin(Bop::Div, _, _) => safe = false,
                        _ => {}
                    });
                    if !safe { blocked.insert(n.clone()); }
                    safe
                } else { false };
                if movable { self.blocks[i].body.push(s); } else { if let Some(e) = s.parts().0 { e.walk(&mut |e| if let E::V(n) = e { prior_reads.insert(n.clone()); }); } kept.push(s); }
            }
            self.blocks[next].body = kept;
        }
    }
    fn unchanged(&self) -> BTreeSet<String> {
        let mut defs: BTreeMap<String, Vec<E>> = BTreeMap::new();
        let mut origins: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for ps in self.params.values() {
            for p in ps { origins.entry(p.clone()).or_default().insert(format!("param:{p}")); }
        }
        for b in &self.blocks {
            for s in &b.body {
                if let S::Set(n, e) = s { defs.entry(n.clone()).or_default().push(e.clone()); }
            }
            if let End::Call(f, args, outs, _) = &b.end {
                for (n, e) in self.params[f].iter().zip(args) { defs.entry(n.clone()).or_default().push(e.clone()); }
                for n in outs { origins.entry(n.clone()).or_default().insert(format!("result:{n}")); }
            }
        }
        loop {
            let old = origins.clone();
            for (n, es) in &defs {
                for e in es {
                    let values = match e {
                        E::V(v) => old.get(v).cloned().unwrap_or_default(),
                        E::Int(i, t) => BTreeSet::from([format!("constant:{i}:{t:?}")]),
                        _ => BTreeSet::from([format!("computed:{n}")]),
                    };
                    origins.entry(n.clone()).or_default().extend(values);
                }
            }
            if old == origins { break; }
        }
        origins.into_iter().filter_map(|(n, values)| {
            (values.len() == 1 && values.iter().all(|v| v.starts_with("param:") || v.starts_with("constant:"))).then_some(n)
        }).collect()
    }
    fn incoming(&self) -> Vec<usize> {
        let mut counts = vec![0; self.blocks.len()];
        for b in &self.blocks { for j in b.end.targets() { counts[j] += 1; } }
        counts
    }
    /// Fresh call-boundary facts: only values live after the call, excluding
    /// results and invariant inputs. Recompute after every graph mutation.
    fn captures(&self) -> BTreeMap<usize, Vec<String>> {
        let live = self.live();
        let unchanged = self.unchanged();
        self.blocks.iter().enumerate().filter_map(|(i, b)| {
            let End::Call(_, _, outs, next) = &b.end else { return None };
            Some((i, live[*next].iter().filter(|n| !outs.contains(n) && !unchanged.contains(*n)).cloned().collect()))
        }).collect()
    }
    fn live(&self) -> Vec<BTreeSet<String>> {
        let mut live = vec![BTreeSet::new(); self.blocks.len()];
        loop {
            let mut changed = false;
            for (i, b) in self.blocks.iter().enumerate().rev() {
                let mut next: BTreeSet<_> = b.end.targets().into_iter().flat_map(|j| live[j].iter().cloned()).collect();
                if let End::Call(_, _, outs, _) = &b.end { for n in outs { next.remove(n); } }
                let read = |e: &E, next: &mut BTreeSet<String>| next.extend(e.reads().into_iter().filter(|n| self.locals.contains_key(n)));
                for e in b.end.exprs() { read(e, &mut next); }
                for s in b.body.iter().rev() {
                    if let S::Set(n, _) = s { next.remove(n); }
                    if let S::Let(p, ..) = s { for n in pat_names(p) { next.remove(&n); } }
                    if let Some(e) = s.parts().0 { read(e, &mut next); }
                }
                if next != live[i] { live[i] = next; changed = true; }
            }
            if !changed { return live; }
        }
    }
}
fn width(t: Ty) -> usize { if let Ty::Tup(k) = t { k } else { 1 } }
fn jump(pc: usize) -> Vec<S> { vec![S::Jump(u64_(pc as u64))] }
fn pop() -> E { p("native_pop", vec![E::Ref("frames".into())]) }

// Rewrite only structured branch paths; loop and function scopes stay intact.
fn map_control(body: &[S], f: &mut dyn FnMut(&S) -> Option<Vec<S>>) -> Option<Vec<S>> {
    let mut out = Vec::new();
    for s in body {
        let mut s = s.clone();
        if matches!(s, S::If(..) | S::Switch(..)) {
            for child in s.parts_mut().1 { *child = map_control(child, f)?; }
        }
        out.extend(f(&s)?);
    }
    Some(out)
}

// Keep single-entry control paths structured instead of redispatching every
// branch. Call entries and return addresses remain dispatch destinations.
fn structure(arms: Vec<(u64, Vec<S>)>, roots: &BTreeSet<u64>, loops: &BTreeSet<usize>) -> Vec<(u64, Vec<S>)> {
    fn targets(body: &[S], counts: &mut BTreeMap<u64, usize>) {
        walk_stmts(body, &mut |s| if let S::Jump(E::Int(i, _)) = s { *counts.entry(*i as u64).or_default() += 1; });
    }
    let mut counts = BTreeMap::new();
    for (_, b) in &arms { targets(b, &mut counts); }
    let inline: BTreeMap<_, _> = arms.iter().filter(|(k, _)| counts.get(k) == Some(&1) && !roots.contains(k)).cloned().collect();
    fn expand(body: &[S], inline: &BTreeMap<u64, Vec<S>>) -> Vec<S> {
        map_control(body, &mut |s| {
            if let S::Jump(E::Int(k, _)) = s {
                if let Some(b) = inline.get(&(*k as u64)) { return Some(expand(b, inline)); }
            }
            Some(vec![s.clone()])
        }).unwrap()
    }
    fn backedges(body: &mut [S], head: u64) {
        mutate_stmts(body, &mut |s| {
            if matches!(s, S::Jump(E::Int(k, _)) if *k as u64 == head) { *s = S::Continue; }
            matches!(s, S::If(..) | S::Switch(..))
        });
    }
    arms.iter().filter(|(k, _)| !inline.contains_key(k) && (roots.contains(k) || counts.contains_key(k))).map(|(k, b)| {
        let mut body = expand(b, &inline);
        if loops.contains(&(*k as usize)) { backedges(&mut body, *k); body = vec![S::Loop(body)]; }
        (*k, body)
    }).collect()
}

// A native region with one loop and acyclic entry/return paths needs no
// dispatch. Inline those paths at their edges; every backedge stays a continue.
fn structured_cycle(arms: &[(u64, Vec<S>)], entry: u64) -> Option<Vec<S>> {
    let loops: Vec<_> = arms.iter().filter(|(_, b)| matches!(b.as_slice(), [S::Loop(_)])).collect();
    if loops.len() != 1 { return None; }
    let (head, body) = loops[0];
    let S::Loop(body) = &body[0] else { unreachable!() };
    let mut edges = BTreeMap::new();
    for (id, path) in arms.iter().filter(|(id, _)| id != head) {
        let Some((S::Jump(E::Int(to, _)), prefix)) = path.split_last() else { return None };
        if prefix.iter().any(|s| { fn control(s: &S) -> bool { matches!(s, S::Jump(_) | S::Continue | S::Loop(_) | S::Ret(_)) || s.parts().1.into_iter().flatten().any(control) } control(s) }) { return None; }
        edges.insert(*id, (prefix.to_vec(), *to as u64));
    }
    let mut paths = BTreeMap::new();
    for id in edges.keys() {
        let mut path = Vec::new();
        let mut at = *id;
        let mut seen = BTreeSet::new();
        while at != *head {
            if !seen.insert(at) { return None; }
            let (prefix, next) = edges.get(&at)?;
            path.extend(prefix.clone());
            at = *next;
        }
        paths.insert(*id, path);
    }
    let mut out = paths.get(&entry)?.clone();
    out.push(S::Loop(map_control(body, &mut |s| Some(match s {
        S::Jump(E::Int(to, _)) => {
            let mut path = if *to as u64 == *head { vec![] } else { paths.get(&(*to as u64))?.clone() };
            path.push(S::Continue);
            path
        }
        S::Jump(_) | S::Loop(_) => return None,
        s => vec![s.clone()],
    }))?));
    Some(out)
}

// Native calls never inspect fuel. Share their compiler-owned work counter;
// only the enclosing dive can observe the charge, after the region returns.
fn shared_work(body: &mut Vec<S>, charged: bool) -> bool {
    let charge = |s: &S| matches!(s, S::Do(E::Call { f, args, .. }) if f == "work_fuel" && *args == vec![v("fuel"), v("fl")]);
    retain_rewrite(body, &mut |s| !charge(s) && (charged || !matches!(s, S::Let(Pat::One(n), Ty::I64, E::Int(1, Ty::I64)) if n == "fl")), &mut |_| {});
    let mut safe = true;
    mutate_stmts(body, &mut |s| {
        if matches!(s, S::Let(Pat::One(n), Ty::I64, E::Int(1, Ty::I64)) if n == "fl")
            || matches!(s, S::Set(n, E::Bin(Bop::Add, a, b)) if n == "fl" && **a == v("fl") && **b == i64_(1)) {
            *s = set("native_work", bin(Bop::Add, v("native_work"), i64_(1)));
            return false;
        }
        let (expr, children) = s.parts();
        if let Some(e) = expr { e.walk(&mut |e| match e {
            E::V(n) if n == "fl" => safe = false,
            E::Call { f, args, .. } if args.iter().any(|a| *a == v("fuel")) && !f.starts_with("s_") => safe = false,
            E::Deref(n) | E::Ref(n) | E::Addr(n) if n == "fuel" => safe = false,
            _ => {}
        }); }
        let structured = children.is_empty() || matches!(s, S::If(..) | S::Switch(..) | S::Loop(_));
        safe &= structured;
        structured
    });
    safe
}

thread_local! {
    /// Physical frames for native recursion. A device lane has a small fixed
    /// stack, so a recursive call component runs as one dispatch loop over
    /// explicit frames; a CPU thread's stack holds the recursion directly (the
    /// same logical calls and returns, no frame traffic). Rust emission clears it.
    pub(crate) static FRAMES: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Native functions in a recursive call component share a dispatch loop.
/// Nonrecursive functions and the dive/rule forms retain their existing lowering.
pub(crate) fn lower(module: &mithril_front::core::CoreModule, fns: &mut Vec<FnDef>, plans: &BTreeMap<u32, FoldPlan>, joins: &[u16], fwd: u16) -> Vec<u32> {
    let bounds: BTreeMap<_, _> = crate::bounds::analyze(module).into_iter().enumerate().map(|(fid, vars)| {
        let vars = vars.into_iter().map(|v| format!("v{v}")).collect();
        (format!("s_{fid}"), vars)
    }).collect();
    let mut entries = Vec::new();
    for (name, mut group) in components(fns) {
        let name = &name;
        let mut shared = group.clone();
        let coalesced = shared.values_mut().all(|f| {
            fn charges(body: &[S]) -> bool { body.iter().any(|s| matches!(s, S::Do(E::Call { f, .. }) if f == "work_fuel") || s.parts().1.into_iter().any(charges)) }
            let charged = charges(&f.body);
            shared_work(&mut f.body, charged)
        });
        if coalesced { group = shared; }
        // Single-entry expansion is valid only if removing that entry breaks
        // every call cycle. Other components keep all their native entries.
        let single_entry = single_entry(&group, name);
        let mut m = Machine::new(&group, &bounds, single_entry.then_some(name.as_str()));
        let fid: u32 = name[2..].parse().unwrap();
        if let Some(plan) = plans.get(&fid) { fns.push(folds::expand(&m, name, plan, joins[fid as usize], fwd)); }
        if !FRAMES.with(|f| f.get()) {
            continue;
        }
        m.tail_calls();
        m.hoist_captures();
        if coalesced { exits::complete(&mut m); }
        joins::share(&mut m);
        paths::copies(&mut m);
        paths::selectors(&mut m);
        let captures = m.captures();
        let single_return = m.blocks.iter().filter_map(|b| if let End::Call(_, _, _, next) = b.end { Some(next) } else { None }).count() == 1;
        let mut resume_to = 0;
        let returns = if single_entry { width(group[name].ret) } else { group.values().map(|f| width(f.ret)).max().unwrap() };
        let nargs = group.values().map(|f| f.params.len() - 1).max().unwrap();
        let machine_name = format!("native_{name}");
        let ctx = group.values().any(|f| f.ctx);
        let mut arms = Vec::new();
        let mut frame_width = 0;
        let mut roots: BTreeSet<_> = m.entries.values().map(|i| *i as u64).collect();
        let result = || E::Tup((0..returns).map(|i| v(format!("r{i}"))).collect());
        for (i, block) in m.blocks.iter().enumerate() {
            let mut body = block.body.clone();
            match &block.end {
                End::Return(es) => {
                    body.extend(es.iter().enumerate().map(|(i, e)| set(format!("r{i}"), e.clone())));
                    let mut finish = Vec::new();
                    if coalesced { finish.push(do_(p("native_work_fuel", vec![v("fuel"), v("native_work")]))); }
                    finish.extend([do_(p("native_done", vec![E::Ref("frames".into())])), ret(result())]);
                    body.push(S::If(p("native_empty", vec![E::Addr("frames".into())]), finish, vec![]));
                    body.push(S::Jump(if single_return { u64_(0) } else { pop() }));
                }
                End::Call(f, args, outs, next) => {
                    let saved = &captures[&i];
                    // Arguments must be evaluated before the callee overwrites its parameters.
                    for (j, e) in args.iter().enumerate() { body.push(let_(format!("a{j}"), Ty::I64, e.clone())); }
                    if coalesced {
                        for exit in exits::leaf(&m, f) {
                            let mut fast = vec![set("native_work", bin(Bop::Add, v("native_work"), exit.work))];
                            fast.extend(outs.iter().zip(exit.values).map(|(n, e)| set(n, e)));
                            fast.extend(jump(*next));
                            body.push(S::If(exit.condition, fast, vec![]));
                        }
                        if width(group[f].ret) == returns {
                            for exit in exits::tail(&m, *next, outs) {
                                let mut fast = vec![set("native_work", bin(Bop::Add, v("native_work"), exit.work))];
                                fast.extend(m.params[f].iter().enumerate().map(|(j,n)| set(n,v(format!("a{j}")))));
                                fast.extend(jump(m.entries[f]));
                                body.push(S::If(exit.condition, fast, vec![]));
                            }
                        }
                    }
                    let frame = storage::Frame::new(&m, saved);
                    let data_width = frame.width;
                    let total_width = data_width + if single_return { 0 } else { 2 };
                    if single_return { frame_width = total_width; }
                    body.push(let_("save_slot", Ty::U64, p("native_reserve", vec![E::Ref("frames".into()), u32_(total_width as u64)])));
                    body.push(frame.transfer(false));
                    let resume = m.blocks.len() + arms.iter().filter(|(id, _)| *id >= m.blocks.len() as u64).count();
                    roots.insert(resume as u64);
                    resume_to = resume;
                    if !single_return { body.push(do_(p("native_set64", vec![E::Ref("frames".into()), v("save_slot"), u32_(data_width as u64), u64_(resume as u64)]))); }
                    body.push(S::If(E::Not(Box::new(p("native_check", vec![v("save_slot")]))), vec![ret(E::Tup(vec![i64_(0); returns]))], vec![]));
                    body.extend(m.params[f].iter().enumerate().map(|(j, n)| set(n, v(format!("a{j}")))));
                    body.extend(jump(m.entries[f]));
                    let mut back = vec![let_("restore_slot", Ty::U64, p("native_take", vec![E::Ref("frames".into()), u32_(data_width as u64)]))];
                    back.push(frame.transfer(true));
                    back.push(do_(p("native_release", vec![E::Ref("frames".into()), v("restore_slot")])));
                    back.extend(outs.iter().enumerate().map(|(j, n)| set(n, v(format!("r{j}")))));
                    back.extend(jump(*next));
                    arms.push((resume as u64, back));
                }
                end => body.push(end.control().unwrap()),
            }
            arms.push((i as u64, body));
        }
        if single_return {
            fn fix(body: &mut [S], resume: usize) {
                mutate_stmts(body, &mut |s| {
                    if matches!(s, S::Jump(E::Int(0, Ty::U64))) { *s = S::Jump(u64_(resume as u64)); }
                    matches!(s, S::If(..))
                });
            }
            for (id, body) in &mut arms {
                if *id < m.blocks.len() as u64 && matches!(m.blocks[*id as usize].end, End::Return(_)) { fix(body, resume_to); }
            }
        }
        let mut body: Vec<_> = m.locals.iter().map(|(n, t)| let_(n, *t, E::Int(0, *t))).collect();
        body.extend((0..returns).map(|i| let_(format!("r{i}"), Ty::I64, i64_(0))));
        if coalesced { body.push(let_("native_work", Ty::I64, i64_(0))); }
        let resident = if coalesced { storage::resident(&m, name, returns) } else { None };
        if resident.is_none() { body.push(let_("frames", Ty::Frames, p("native_frames", vec![u32_(frame_width as u64)]))); }
        body.push(S::Switch(v("pc"), m.entries.keys().map(|n| (m.entries[n] as u64, m.params[n].iter().enumerate().map(|(j, p)| set(p, v(format!("a{j}")))).collect())).collect(), None));
        let mut arms = structure(arms, &roots, &m.loops);
        for (_, path) in &mut arms { paths::factor(path); borrowed_cells(path); }
        if let Some(resident) = resident { body.extend(resident); }
        else if single_entry && single_return {
            body.extend(structured_cycle(&arms, m.entries[name] as u64).unwrap_or_else(|| vec![S::Machine(arms)]));
        } else { body.push(S::Machine(arms)); }
        storage::compact(&mut body, &m.narrow);
        let mut params = vec![("fuel".into(), Ty::RefI64), ("pc".into(), Ty::U64)];
        params.extend((0..nargs).map(|j| (format!("a{j}"), Ty::I64)));
        for f in fns.iter_mut().filter(|f| m.entries.contains_key(&f.name)) {
            let fid: u32 = f.name[2..].parse().unwrap();
            if !f.ctx && f.params.len() == module.fns[fid as usize].arity + 1 && f.params.iter().skip(1).all(|(_,t)| *t==Ty::I64) {
                entries.push(fid);
            }
            let mut args = vec![v("fuel"), u64_(m.entries[&f.name] as u64)];
            args.extend(f.params.iter().skip(1).map(|(n, _)| v(n)));
            args.resize(nargs + 2, i64_(0));
            let names: Vec<_> = (0..returns).map(|i| format!("r{i}")).collect();
            let val = if f.ret == Ty::I64 { v("r0") } else { E::Tup(names.iter().take(width(f.ret)).map(v).collect()) };
            f.body = vec![S::Let(Pat::Tup(names), Ty::Infer, E::Call { f: machine_name.clone(), ctx, args }), ret(val)];
        }
        fns.push(FnDef { name: machine_name, ctx, params, ret: Ty::Tup(returns), body, inline: Inline::Default, cold: false });
    }
    entries
}

// Keep both executions of innermost recursive value regions. Growing
// exposes the same calls as redexes; a populated frontier executes locally.
pub(crate) fn regions(m: &mithril_front::core::CoreModule, all: &[Option<crate::scalar::Sig>], scal: &[Option<crate::scalar::Sig>], bor: &[Vec<bool>], tys: &crate::ty::Types, plans: &BTreeMap<u32, FoldPlan>, fns: &mut Vec<FnDef>) {
    use crate::scalar::{self, Kind, PTy};
    use mithril_front::core::Core;
    let mut reach: Vec<BTreeSet<u32>> = m.fns.iter().map(|f| crate::callees(&f.body).into_iter().collect()).collect();
    loop {
        let old = reach.clone();
        for cs in &mut reach { for g in cs.clone() { cs.extend(old[g as usize].iter().copied()); } }
        if reach == old { break; }
    }
    let local = |f: usize| all[f].as_ref().is_some_and(|s| s.params.iter().all(|p| matches!(p, PTy::I | PTy::H)) && !s.ra.iter().any(|a| *a) && match s.ret { Kind::S1 => tys.ret[f] == crate::ty::Ty::Int, Kind::SK(k) => tys.ret[f] == crate::ty::Ty::Tup(k as u32), Kind::No => false });
    let regions: Vec<bool> = (0..m.fns.len()).map(|f| {
        scal[f].is_none() && reach[f].contains(&(f as u32)) && local(f)
            && reach[f].iter().any(|g| reach[*g as usize].contains(&(f as u32)) && (crate::fork_recursive(*g, &m.fns[*g as usize]) || plans.contains_key(g)))
            && reach[f].iter().all(|g| !reach[*g as usize].contains(g) || reach[*g as usize].contains(&(f as u32)))
    }).collect();
    // Acyclic integer callers may run locally when the aggregate downstream
    // recursive frontier is wide enough. Their original dives still grow it.
    let pure: Vec<bool> = (0..m.fns.len()).map(|f| local(f) && !m.fns[f].body.any(&mut |e| match e {
        Core::Flo(_) | Core::Ctor(..) | Core::Reuse(..) | Core::Lam(..) | Core::App(..) => Some(true),
        Core::Prim(p, _) if !p.is_f32() => Some(true),
        _ => None,
    })).collect();
    let mut selected = regions.clone();
    loop {
        let old = selected.clone();
        for f in 0..m.fns.len() {
            if scal[f].is_none() && pure[f] && !reach[f].contains(&(f as u32)) && (f as u32 == m.main || reach.iter().any(|cs| cs.contains(&(f as u32)))) && reach[f].iter().any(|g| regions[*g as usize])
                && reach[f].iter().all(|g| pure[*g as usize] && (scal[*g as usize].is_some() || old[*g as usize])) { selected[f] = true; }
        }
        if selected == old { break; }
    }
    let mut sigs = scal.to_vec();
    for (f, yes) in selected.iter().enumerate() { if *yes { sigs[f] = all[f].clone(); } }
    if selected.iter().all(|s| !s) { return; }
    let old_ctx = scalar::CTX.with(|c| c.replace(scalar::needs_ctx(m, &sigs)));
    let old_leaf = scalar::LEAF.with(|l| l.replace(m.fns.iter().map(|f| !crate::any_call(&f.body)).collect()));
    let old_bridge = scalar::BRIDGE_LIVE.with(|b| b.replace(vec![true; m.fns.len()]));
    for (fid, yes) in selected.iter().enumerate() {
        if !yes { continue; }
        let mut forms = scalar::scalar_fn(m, fid as u32, &sigs, bor, true);
        let mut native = forms.remove(0);
        let bridge = forms.remove(0);
        // A borrowed traversal runs within the owner's WORK task. It need
        // not materialize its child calls to populate a second frontier.
        let component: Vec<E> = if sigs[fid].as_ref().unwrap().params.contains(&PTy::H) { vec![] } else {
            (0..m.fns.len()).filter(|g| regions[*g] && reach[fid].contains(&(*g as u32))).map(|g| u64_((1 + g) as u64)).collect()
        };
        let ready = c("native_ready", vec![E::Slice(component)]);
        let mut fasts = BTreeMap::from([(format!("d_{fid}"), bridge.body)]);
        if let Kind::SK(k) = sigs[fid].as_ref().unwrap().ret {
            fasts.insert(format!("n_{fid}"), scalar::port_bridge(fid as u32, sigs[fid].as_ref().unwrap(), &bor[fid], Ty::ResArr(k)));
        }
        let tuple_bridge = !regions[fid] && fns.iter().any(|f| f.name == format!("n_{fid}") && f.ret == Ty::ResArr(width(native.ret)));
        let mut fallbacks = Vec::new();
        let mut prefixes = Vec::new();
        for f in fns.iter_mut() {
            if tuple_bridge && f.name == format!("d_{fid}") { continue; }
            let Some(fast) = fasts.get(&f.name) else { continue };
            if let (Ty::ResArr(k), Kind::SK(n)) = (f.ret, sigs[fid].as_ref().unwrap().ret) { if k != n { continue; } }
            if plans.contains_key(&(fid as u32)) {
                let args = std::iter::once(v("fuel")).chain((0..m.fns[fid].arity).map(|i| p("take_i", vec![v(format!("v{i}"))]))).collect();
                f.body = vec![S::If(ready.clone(), fast.clone(), vec![]),
                    let_("region_mode", Ty::Bool, c("native_enter", vec![])),
                    let_("region_root", Ty::U32, c(&format!("expand_s_{fid}"), args)),
                    do_(c("native_leave", vec![v("region_mode")])), ret(E::Err(Box::new(cast(v("region_root"), Ty::U64))))];
                continue;
            }
            let mut slow = f.clone();
            slow.name = format!("region_{}", f.name);
            if !regions[fid] { if let Some(prefix) = prefix::share(&mut native, &mut slow, m, &sigs) { prefixes.push(prefix); } }
            let args = f.params.iter().map(|(n, _)| v(n)).collect();
            f.body = vec![S::If(ready.clone(), fast.clone(), vec![]),
                let_("region_mode", Ty::Bool, c("native_enter", vec![])),
                let_("region_value", f.ret, E::Call { f: slow.name.clone(), ctx: slow.ctx, args }),
                do_(c("native_leave", vec![v("region_mode")])), ret(v("region_value"))];
            fallbacks.push(slow);
        }
        fns.extend(fallbacks);
        fns.extend(prefixes);
        fns.push(native);
    }
    scalar::CTX.with(|c| *c.borrow_mut() = old_ctx);
    scalar::LEAF.with(|l| *l.borrow_mut() = old_leaf);
    scalar::BRIDGE_LIVE.with(|b| *b.borrow_mut() = old_bridge);
}

#[path = "native_prefix.rs"]
mod prefix;

#[path = "native_store.rs"]
mod storage;

#[path = "native_exit.rs"]
mod exits;

#[path = "native_paths.rs"]
mod paths;

#[path = "native_join.rs"]
mod joins;

#[path = "native_fold.rs"]
mod folds;
pub(crate) use folds::{discover, FoldPlan};

#[cfg(test)]
mod fold_tests {
    use super::*;
    fn accepted(body: &str, seed: &str) -> bool {
        let source = format!("{body}\ndef main():\n    return f(4, {seed}, 0)\n");
        let m = mithril_front::desugar(&mithril_front::parse(&source).unwrap()).unwrap();
        let analysis = crate::Lowering::new(&m);
        let fid = analysis.module.fns.iter().position(|f| f.name == "f").unwrap();
        discover(&analysis.module, &analysis.native_bodies()).contains_key(&(fid as u32))
    }
    #[test]
    fn recursive_tree_threads_only_the_two_additive_counters() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let src = std::fs::read_to_string(root.join("crates/mithril-codegen/tests/fixtures/recursive_counts.py")).unwrap();
        let m = mithril_front::desugar(&mithril_front::parse(&src).unwrap()).unwrap();
        let analysis = crate::Lowering::new(&m);
        let fid = analysis.module.fns.iter().position(|f| f.name == "solve").unwrap() as u32;
        let plans = discover(&analysis.module, &analysis.native_bodies());
        assert_eq!(plans.get(&fid).map(|p| p.seeds.clone()), Some(vec![5, 6]));
    }
    const FOLD: &str = "def f(n, a, b):\n    while n > 0:\n        w = f(n - 1, a, (b + 1) & 4294967295)\n        a = w[0]\n        b = w[1]\n        n = n - 1\n    return (a, b)\n";
    #[test]
    fn additive_recursive_fold_has_an_independent_summary() {
        assert!(accepted(FOLD, "0"));
        assert!(accepted(FOLD, "4294967295"));
    }
    #[test]
    fn summary_must_preserve_each_child_and_ignore_it_in_control() {
        assert!(!accepted(FOLD, "-1"));
        assert!(!accepted(&FOLD.replace("a = w[0]", "a = (w[0] * 2 - a) & 4294967295"), "0"));
        assert!(!accepted(&FOLD.replace("a = w[0]", "a = a"), "0"));
        assert!(!accepted(&FOLD.replace("        w =", "        old = a\n        w =").replace("n = n - 1", "n = w[0] - old"), "0"));
        assert!(!accepted(&FOLD.replace("while n > 0", "while a > 0"), "0"));
    }
    #[test]
    fn modulo_zero_coefficients_do_not_prove_control_independence() {
        let body = "def f(n, a, b):\n    while n > 0:\n        if a * 4294967296 == 0:\n            b = (b + 1) & 4294967295\n        w = f(n - 1, (a + 1) & 4294967295, b)\n        a = w[0]\n        b = w[1]\n        n = n - 1\n    return (a, b)\n";
        for seed in ["0", "1", "4294967295"] {
            assert!(!accepted(body, seed), "seed={seed}");
        }
    }

}

#[cfg(test)]
#[path = "../tests/support/native_control.rs"]
mod control_tests;

#[cfg(test)]
#[path = "../tests/support/native_facts.rs"]
mod facts_tests;
