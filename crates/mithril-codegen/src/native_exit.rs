//! Return paths that can run without allocating a continuation.
use super::*;

pub(super) struct Exit { pub condition: E, pub values: Vec<E>, pub work: E }

fn substitute(e: &E, env: &Env) -> Option<E> {
    if !operations::Policy::Exit.accepts(e) { return None; }
    e.substitute(env, true)
}
fn guard(a: E, b: E) -> E { if a == E::Bool(true) { b } else { bin(Bop::And,a,b) } }
pub(super) fn exits(m: &Machine<'_>, start: usize, mut env: Env) -> Vec<Exit> {
    fn visit(m: &Machine<'_>, at: usize, mut env: Env, condition: E, mut seen: BTreeSet<usize>, out: &mut Vec<Exit>) -> Option<()> {
        if !seen.insert(at) { return Some(()); }
        for s in &m.blocks[at].body {
            match s {
                S::Set(n,e) => { let e=substitute(e, &env)?; env.insert(n.clone(),e); }
                S::Comment(_) => {}
                _ => return None,
            }
        }
        match &m.blocks[at].end {
            End::Return(es) => out.push(Exit { condition,values:es.iter().map(|e| substitute(e, &env)).collect::<Option<_>>()?,work:env["native_work"].clone() }),
            End::Jump(j) => { visit(m,*j,env,condition,seen,out)?; }
            End::Branch(e,a,b) => {
                let e=substitute(e, &env)?;
                // Either unsupported path may fall back to its original body.
                let _ = visit(m,*a,env.clone(),guard(condition.clone(),e.clone()),seen.clone(),out);
                let _ = visit(m,*b,env,guard(condition,E::Not(Box::new(e))),seen,out);
            }
            End::Switch(e, arms, default) => {
                let e = substitute(e, &env)?;
                let mut otherwise = condition.clone();
                for (key, target) in arms {
                    let test = bin(Bop::Eq, cast(e.clone(), Ty::U64), u64_(*key));
                    let _ = visit(m, *target, env.clone(), guard(condition.clone(), test.clone()), seen.clone(), out);
                    otherwise = guard(otherwise, E::Not(Box::new(test)));
                }
                if let Some(target) = default { let _ = visit(m, *target, env, otherwise, seen, out); }
            }
            _ => {}
        }
        Some(())
    }
    env.insert("native_work".into(),i64_(0));
    let mut out=Vec::new();
    let _ = visit(m,start,env,E::Bool(true),BTreeSet::new(),&mut out);
    out
}
pub(super) fn leaf(m: &Machine<'_>, name: &str) -> Vec<Exit> {
    let env=m.params[name].iter().enumerate().map(|(i,n)| (n.clone(),v(format!("a{i}")))).collect();
    exits(m,m.entries[name],env)
}
pub(super) fn tail(m: &Machine<'_>, next: usize, outs: &[String]) -> Vec<Exit> {
    let children: Vec<_>=(0..outs.len()).map(|i| v(format!("child_result_{i}"))).collect();
    let mut env: Env=m.locals.keys().map(|n| (n.clone(),v(n))).collect();
    env.extend(outs.iter().cloned().zip(children.iter().cloned()));
    exits(m,next,env).into_iter().filter(|e| {
        let independent=|e: &E| e.reads().iter().all(|n| !n.starts_with("child_result_"));
        e.values==children && independent(&e.condition) && independent(&e.work)
    }).collect()
}

/// Complete total call exits at their original control positions, before call
/// sharing erases the acyclic order of sibling continuations.
pub(super) fn complete(m: &mut Machine<'_>) {
    let mut changes = Vec::new();
    for at in 0..m.blocks.len() {
        let End::Call(f,args,outs,next) = m.blocks[at].end.clone() else { continue };
        let exits = leaf(m,&f);
        if exits.is_empty() { continue; }
        changes.push((at,f,args,outs,next,exits));
    }
    for (at,f,args,outs,next,exits) in changes {
        let temps:Vec<_>=args.iter().map(|_|m.local(Ty::I64)).collect();
        m.blocks[at].body.extend(temps.iter().zip(args).map(|(n,e)|set(n,e)));
        let env:Env=temps.iter().enumerate().map(|(i,n)|(format!("a{i}"),v(n))).collect();
        let fallback=m.block();
        m.blocks[fallback].end=End::Call(f,temps.iter().map(v).collect(),outs.clone(),next);
        let mut current=at;
        let count=exits.len();
        let mut failed=Vec::new();
        for (i,exit) in exits.into_iter().enumerate() {
            let fast=m.block();
            m.blocks[fast].body.push(set("native_work",bin(Bop::Add,v("native_work"),exit.work.substitute(&env,false).unwrap())));
            m.blocks[fast].body.extend(outs.iter().zip(exit.values).map(|(n,e)|set(n,e.substitute(&env,false).unwrap())));
            m.blocks[fast].end=End::Jump(next);
            let slow=if i+1==count {fallback}else{m.block()};
            let mut condition=exit.condition.clone();
            condition.rewrite(&mut |e| match e {
                E::Not(x) if failed.contains(&**x) => *e=E::Bool(true),
                E::Bin(Bop::And,a,b) if **a==E::Bool(true) => *e=(**b).clone(),
                E::Bin(Bop::And,a,b) if **b==E::Bool(true) => *e=(**a).clone(),
                _=>{},
            });
            failed.push(exit.condition);
            m.blocks[current].end=End::Branch(condition.substitute(&env,false).unwrap(),fast,slow);
            current=slow;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn machine(blocks: Vec<Block>, locals: &[&str]) -> Machine<'static> {
        Machine { funcs: Box::leak(Box::new(BTreeMap::new())), blocks,
            locals: locals.iter().map(|n| (n.to_string(), Ty::I64)).collect(),
            params: BTreeMap::new(), entries: BTreeMap::new(), active: vec![], loops: BTreeSet::new(),
            bounds: Box::leak(Box::new(BTreeMap::new())), narrow: BTreeSet::new() }
    }
    #[test]
    fn return_paths_preserve_values_guards_and_work() {
        let m = machine(vec![
            Block { body: vec![set("native_work", bin(Bop::Add, v("native_work"), i64_(1)))], end: End::Branch(bin(Bop::Eq,v("n"),i64_(0)),1,2) },
            Block { body: vec![set("x",bin(Bop::Add,v("a"),i64_(3)))], end: End::Return(vec![v("x")]) },
            Block { body: vec![], end: End::Call("s_0".into(),vec![v("n")],vec!["x".into()],1) },
        ], &["n","a","x"]);
        let paths = exits(&m,0,BTreeMap::from([("n".into(),v("arg")),("a".into(),v("seed"))]));
        assert_eq!(paths.len(),1);
        assert_eq!(paths[0].condition,bin(Bop::Eq,v("arg"),i64_(0)));
        assert_eq!(paths[0].values,vec![bin(Bop::Add,v("seed"),i64_(3))]);
        assert_eq!(paths[0].work,bin(Bop::Add,i64_(0),i64_(1)));
    }
    #[test]
    fn cycles_effects_and_uninitialized_values_have_no_shortcut() {
        for block in [
            Block { body: vec![], end: End::Jump(0) },
            Block { body: vec![do_(c("arr_set_u",vec![v("a")]))], end: End::Return(vec![v("a")]) },
            Block { body: vec![], end: End::Return(vec![v("missing")]) },
        ] {
            assert!(exits(&machine(vec![block], &["a"]),0,BTreeMap::new()).is_empty());
        }
    }
    #[test]
    fn only_identity_returns_with_child_independent_guards_are_tail_calls() {
        let outs=vec!["x".to_string(),"y".to_string()];
        for (guard,values,expected) in [
            (bin(Bop::Eq,v("n"),i64_(0)),vec![v("x"),v("y")],1),
            (bin(Bop::Eq,v("x"),i64_(0)),vec![v("x"),v("y")],0),
            (bin(Bop::Eq,v("n"),i64_(0)),vec![v("y"),v("x")],0),
            (bin(Bop::Eq,v("n"),i64_(0)),vec![bin(Bop::Add,v("x"),i64_(1)),v("y")],0),
        ] {
            let m=machine(vec![
                Block { body:vec![],end:End::Branch(guard,1,2) },
                Block { body:vec![],end:End::Return(values) },
                Block { body:vec![],end:End::Jump(2) },
            ], &["n","x","y"]);
            assert_eq!(tail(&m,0,&outs).len(),expected);
        }
    }

}
