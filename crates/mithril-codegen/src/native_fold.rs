//! Certify additive summaries before splitting a recursive native region.
//! Seed and child-result probes must each reach the result with coefficient
//! one and never reach control or a non-accumulator argument, modulo 2^32.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub(crate) struct FoldPlan { pub seeds: Vec<usize> }
// Modulo coefficients prove the summary law; exact dependencies separately
// prove that resetting seeds cannot change control or child state.
#[derive(Clone, PartialEq)]
struct Affine { coeff: Vec<u32>, dependent: bool }
type Coeff = Option<Affine>;
type State = BTreeMap<String, Coeff>;
fn zero(k: usize) -> Coeff { Some(Affine { coeff: vec![0; k], dependent: false }) }
fn unit(k: usize, i: usize) -> Coeff {
    let mut coeff = vec![0; k]; coeff[i] = 1;
    Some(Affine { coeff, dependent: true })
}
fn coeff(e: &E, s: &State, k: usize) -> Coeff {
    match e {
        E::V(n) => s.get(n).cloned().unwrap_or_else(|| if n == "fuel" { zero(k) } else { None }),
        E::Int(..) | E::Bool(_) => zero(k),
        E::Cast(x, Ty::I64 | Ty::U64 | Ty::U32) => coeff(x, s, k),
        E::Call { f, args, .. } if f == "wrap56" && args.len() == 1 => coeff(&args[0], s, k),
        E::Neg(x) => {
            let mut x = coeff(x,s,k)?;
            x.coeff.iter_mut().for_each(|v| *v = v.wrapping_neg()); Some(x)
        }
        E::Bin(op, a, b) => {
            let (x, y) = (coeff(a, s, k)?, coeff(b, s, k)?);
            let dependent = x.dependent || y.dependent;
            let coeff = match op {
                Bop::Add => x.coeff.into_iter().zip(y.coeff).map(|(a,b)| a.wrapping_add(b)).collect(),
                Bop::Sub => x.coeff.into_iter().zip(y.coeff).map(|(a,b)| a.wrapping_sub(b)).collect(),
                Bop::And if **b == i64_(4294967295) => return Some(x),
                Bop::And if **a == i64_(4294967295) => return Some(y),
                Bop::Mul => {
                    let (value, scale) = if let E::Int(n, _) = &**a { (y, *n) } else if let E::Int(n, _) = &**b { (x, *n) } else {
                        return (!dependent).then(|| zero(k)).flatten();
                    };
                    value.coeff.into_iter().map(|a| a.wrapping_mul(scale as u32)).collect()
                }
                _ => return (!dependent).then(|| zero(k)).flatten(),
            };
            Some(Affine { coeff, dependent })
        }
        _ => {
            let mut independent = true;
            e.walk(&mut |x| if let E::V(n) = x { if s.get(n).is_none_or(|v| v.as_ref().is_none_or(|v| v.dependent)) { independent = false; } });
            independent.then(|| zero(k)).flatten()
        }
    }
}
fn probe(m: &Machine<'_>, seeds: &[usize], start: usize, initial: &[String]) -> bool {
    let k = seeds.len();
    let mut state: State = m.locals.keys().map(|n| (n.clone(), zero(k))).collect();
    for (i, n) in initial.iter().enumerate() { state.insert(n.clone(), unit(k, i)); }
    let mut states = vec![None; m.blocks.len()];
    states[start] = Some(state);
    let mut work = VecDeque::from([start]);
    let mut steps = 0;
    while let Some(at) = work.pop_front() {
        steps += 1;
        if steps > 8192 { return false; }
        let mut s = states[at].clone().unwrap();
        let independent = |e: &E, s: &State| coeff(e, s, k).is_some_and(|v| !v.dependent);
        for stmt in &m.blocks[at].body {
            match stmt {
                S::Set(n, e) => { s.insert(n.clone(), coeff(e, &s, k)); }
                S::Let(p, _, e) => { let value = coeff(e, &s, k); for n in pat_names(p) { s.insert(n, value.clone()); } }
                S::Do(E::Call { f, .. }) if f == "work_fuel" || f == "stack_guard" => {}
                S::Comment(_) => {}
                _ => return false,
            }
        }
        match &m.blocks[at].end {
            End::Branch(e,..) | End::Switch(e,..) if !independent(e, &s) => return false,
            End::Return(es) => {
                if es.len() != k || es.iter().enumerate().any(|(i,e)| coeff(e, &s, k) != unit(k,i)) { return false; }
            }
            End::Call(_, args, outs, _) => {
                if args.iter().enumerate().any(|(i,e)| !seeds.contains(&i) && !independent(e, &s)) || outs.len() != k { return false; }
                let values: Vec<_> = seeds.iter().map(|i| coeff(&args[*i], &s, k)).collect();
                for (n, value) in outs.iter().zip(values) { s.insert(n.clone(), value); }
            }
            _ => {}
        }
        for j in m.blocks[at].end.targets() {
            if let Some(old) = &mut states[j] {
                let mut changed = false;
                for (n, value) in old.iter_mut() {
                    if *value != s.get(n).cloned().unwrap_or(None) && value.is_some() { *value = None; changed = true; }
                }
                if changed { work.push_back(j); }
            } else { states[j] = Some(s.clone()); work.push_back(j); }
        }
    }
    true
}
fn canonical(e: &E, known: &BTreeSet<String>) -> bool {
    match e {
        E::Int(n, _) => (0..=4294967295).contains(n),
        E::V(n) => known.contains(n),
        E::Cast(_, Ty::U32) => true,
        E::Bin(Bop::And, _, b) if **b == i64_(4294967295) => true,
        E::Bin(Bop::Or | Bop::Xor, a, b) => canonical(a, known) && canonical(b, known),
        E::Call { f, args, .. } if f == "wrap56" => canonical(&args[0], known),
        _ => false,
    }
}
fn certificate(m: &Machine<'_>, name: &str, k: usize) -> Option<Vec<usize>> {
    let ps = &m.params[name];
    let mut known = m.narrow.clone();
    let mut defs: BTreeMap<String, Vec<E>> = BTreeMap::new();
    for b in &m.blocks {
        for s in &b.body { if let S::Set(n, e) = s { defs.entry(n.clone()).or_default().push(e.clone()); } }
        if let End::Call(_, _, outs, _) = &b.end { known.extend(outs.iter().cloned()); }
    }
    loop {
        let old = known.len();
        for (n, es) in &defs { if es.iter().all(|e| canonical(e, &known)) { known.insert(n.clone()); } }
        if old == known.len() { break; }
    }
    if m.blocks.iter().any(|b| matches!(&b.end, End::Return(es) if es.iter().any(|e| !canonical(e, &known)))) {
        return None;
    }
    fn search(m: &Machine<'_>, name: &str, k: usize, candidates: &[usize], chosen: &mut Vec<usize>) -> Option<Vec<usize>> {
        if chosen.len() == k {
            let initial: Vec<_> = chosen.iter().map(|i| m.params[name][*i].clone()).collect();
            if !probe(m, chosen, m.entries[name], &initial) { return None; }
            for b in &m.blocks {
                if let End::Call(_, _, outs, next) = &b.end { if !probe(m, chosen, *next, outs) { return None; } }
            }
            return Some(chosen.clone());
        }
        for i in candidates {
            if chosen.contains(i) { continue; }
            chosen.push(*i);
            if let Some(result) = search(m, name, k, candidates, chosen) { return Some(result); }
            chosen.pop();
        }
        None
    }
    // Limit proof search, never execution: unsupported signatures keep their
    // existing lowering. Every accepted coefficient is still checked.
    if k > 4 || ps.len() > 10 { return None; }
    let candidates: Vec<_> = ps.iter().enumerate().filter_map(|(i,n)| m.narrow.contains(n).then_some(i)).collect();
    search(m, name, k, &candidates, &mut vec![])
}

pub(crate) fn discover(module: &mithril_front::core::CoreModule, fns: &[FnDef]) -> BTreeMap<u32, FoldPlan> {
    let bounds: BTreeMap<_,_> = crate::bounds::analyze(module).into_iter().enumerate().map(|(f, ps)| (format!("s_{f}"), ps.into_iter().map(|p| format!("v{p}")).collect())).collect();
    let mut plans = BTreeMap::new();
    for (name, group) in components(fns) {
        let name = &name;
        let fid: u32 = name[2..].parse().unwrap();
        if crate::scalar::shifted(fid) || group.values().any(|f| f.ctx || f.params.iter().skip(1).any(|(_,t)| *t != Ty::I64)) { continue; }
        // The single entry must break the call cycle; reject other internal
        // cycles before expanding their bodies.
        if !single_entry(&group, name) { continue; }
        let m = Machine::new(&group, &bounds, Some(name));
        if !m.blocks.iter().any(|b| matches!(b.end,End::Call(..))) { continue; }
        if let Some(seeds) = certificate(&m,name,width(group[name].ret)) { plans.insert(fid,FoldPlan { seeds }); }
    }
    plans
}

// One expansion visits the region's immediate recursive children, preserving
// their ordering in ordinary binary join records. Child seeds become zero;
// the certified identity responses compute this node's local contribution.
pub(super) fn expand(m: &Machine<'_>, name: &str, plan: &FoldPlan, join: u16, fwd: u16) -> FnDef {
    let fid: u32 = name[2..].parse().unwrap();
    let arity = m.params[name].len();
    let mut arms = Vec::new();
    for (i, b) in m.blocks.iter().enumerate() {
        let mut body = b.body.clone();
        match &b.end {
            End::Return(es) => {
                let ports: Vec<_> = es.iter().cloned().map(num).collect();
                let result = if ports.len() == 1 { ports[0].clone() } else { c("mk_con",vec![u16_(0xfff),E::Slice(ports)]) };
                body.push(do_(p("native_work_fuel",vec![v("fuel"),v("native_work")])));
                body.push(let_("fold_value",Ty::U64,result));
                body.push(do_(c("deliver_deferred",vec![rec_addr("fold_base"),v("fold_value")])));
                body.push(ret(v("fold_top")));
            }
            End::Call(_,args,outs,next) => {
                for (i,e) in args.iter().enumerate() { body.push(let_(format!("child_{i}"),Ty::I64,e.clone())); }
                let ports = (0..arity).map(|i| num(if plan.seeds.contains(&i) { i64_(0) } else { v(format!("child_{i}")) })).collect();
                body.push(let_("fold_join",Ty::U32,c("alloc_rec",vec![u16_(join as u64),u32_(2),u32_(0),u32_(0),E::Const("NONE".into())])));
                body.push(do_(c("set_parent",vec![v("fold_top"),rec_addr("fold_join")])));
                body.push(do_(c("spawn_call",vec![u16_((1+fid) as u64),E::Slice(ports),bin(Bop::Or,rec_addr("fold_join"),u64_(1))])));
                body.push(set("fold_top",v("fold_join")));
                body.extend(outs.iter().zip(&plan.seeds).map(|(n,i)| set(n,v(format!("child_{i}")))));
                body.extend(jump(*next));
            }
            end => body.push(end.control().unwrap()),
        }
        arms.push((i as u64,body));
    }
    let mut body: Vec<_> = m.locals.iter().map(|(n,t)| let_(n,*t,E::Int(0,*t))).collect();
    body.push(let_("native_work",Ty::I64,i64_(0)));
    body.push(let_("fold_base",Ty::U32,c("alloc_rec",vec![u16_(fwd as u64),u32_(1),u32_(0),u32_(0),E::Const("NONE".into())])));
    body.push(let_("fold_top",Ty::U32,v("fold_base")));
    body.extend(m.params[name].iter().enumerate().map(|(i,n)| set(n,v(format!("a{i}")))));
    body.push(let_("pc",Ty::U64,u64_(m.entries[name] as u64)));
    body.push(S::Machine(structure(arms,&BTreeSet::from([m.entries[name] as u64]),&m.loops)));
    let mut params=vec![("fuel".into(),Ty::RefI64)];
    params.extend((0..arity).map(|i| (format!("a{i}"),Ty::I64)));
    FnDef { name:format!("expand_{name}"),ctx:true,params,ret:Ty::U32,body,inline:Inline::Default,cold:false }
}
