//! Choose lossless local storage while preserving each operation's i64 type.
use super::*;

// The same field layout drives saving and restoring. Restores reverse the
// field order, while retaining the exact offsets and each local's type.
pub(super) struct Frame<'a> { fields: Vec<(&'a str, Ty, usize, usize)>, pub width: usize }
impl<'a> Frame<'a> {
    pub fn layout(m: &Machine<'_>, saved: &[String]) -> Vec<(Ty, usize)> {
        saved.iter().map(|n| (m.locals[n], if m.narrow.contains(n) { 1 } else { 2 })).collect()
    }
    pub fn new(m: &Machine<'_>, saved: &'a [String]) -> Self {
        let mut width = 0;
        let fields = saved.iter().zip(Self::layout(m, saved)).map(|(n, (ty, size))| {
            let field = (n.as_str(), ty, width, size);
            width += size;
            field
        }).collect();
        Self { fields, width: width.max(1) }
    }
    pub fn transfer(&self, restore: bool) -> S {
        let slot = if restore { "restore_slot" } else { "save_slot" };
        let fields = |fixed| {
            let operation = if restore { "get" } else { "set" };
            let helper = |size| format!("native_{operation}{}{size}", if fixed { "_fixed" } else { "" });
            let args = |offset| vec![E::Ref("frames".into()), v(slot), u32_(offset)];
            let mut body: Vec<_> = self.fields.iter().map(|(n, ty, offset, size)| {
                let mut args = args(*offset as u64);
                if !restore { args.push(cast(v(*n), if *size == 1 { Ty::U32 } else { Ty::U64 })); }
                let call = p(&helper(size * 32), args);
                if restore { set(*n, cast(call, *ty)) } else { do_(call) }
            }).collect();
            if restore { body.reverse(); }
            if body.is_empty() {
                let mut args = args(0);
                if !restore { args.push(u32_(0)); }
                body.push(do_(p(&helper(32), args)));
            }
            body
        };
        S::If(p("native_cached", vec![E::Addr("frames".into()), v(slot)]), fields(true), fields(false))
    }
}

/// A single recursive call boundary can keep its record alive while sibling
/// calls complete. Acyclic paths retain their original statements and branches.
pub(super) fn resident(m: &Machine<'_>, name: &str, returns: usize) -> Option<Vec<S>> {
    if m.entries.len() != 1 || !m.loops.is_empty() { return None; }
    let calls: Vec<_> = m.blocks.iter().enumerate().filter_map(|(i,b)| {
        if let End::Call(f,args,outs,next) = &b.end { Some((i,f,args,outs,*next)) } else { None }
    }).collect();
    let [(at, callee, args, outs, next)] = calls.as_slice() else { return None };
    if *callee != name { return None; }
    let captures = m.captures();
    let frame = Frame::new(m, &captures[at]);
    if frame.fields.is_empty() { return None; }
    let k = frame.fields.len();
    let shape = || E::Arr(vec![u64_(0);k]);
    let fields = || E::Arr(frame.fields.iter().map(|(n,_,_,_)| cast(v(*n),Ty::U64)).collect());
    let stack = || E::Ref("frames".into());
    let call = |resident| {
    let mut call: Vec<_> = args.iter().enumerate().map(|(j,e)| let_(format!("a{j}"),Ty::I64,e.clone())).collect();
    call.push(do_(p(if resident {"native_record_replace"}else{"native_record_push"},vec![stack(),fields()])));
    call.push(S::If(E::Not(Box::new(p("native_ok",vec![]))),vec![ret(E::Tup(vec![i64_(0);returns]))],vec![]));
    call.extend(m.params[name].iter().enumerate().map(|(j,n)| set(n,v(format!("a{j}")))));
    call };
    let finish = vec![do_(p("native_work_fuel",vec![v("fuel"),v("native_work")])),do_(p("native_record_done",vec![stack()])),ret(E::Tup((0..returns).map(|i|v(format!("r{i}"))).collect()))];
    let returned = |resident| {
    let mut returned = if resident {vec![do_(p("native_record_pop",vec![stack(),shape()]))]}else{vec![]};
    returned.extend([S::If(p("native_record_empty",vec![E::Addr("frames".into())]),finish.clone(),vec![]),
        let_("record",Ty::Arr(k),p("native_record_read",vec![stack(),shape()]))]);
    returned.extend(frame.fields.iter().enumerate().map(|(i,(n,ty,_,_))|set(*n,cast(idx(v("record"),i),*ty))));
    returned.extend(outs.iter().enumerate().map(|(i,n)|set(n,v(format!("r{i}")))));
    returned };
    fn path(m:&Machine<'_>,at:usize,call:&[S],returned:&[S],mut seen:BTreeSet<usize>)->Option<Vec<S>> {
        if !seen.insert(at) { return None; }
        let block=&m.blocks[at];
        let mut body=block.body.clone();
        let child=|j|path(m,j,call,returned,seen.clone());
        match &block.end {
            End::Call(..)=>body.extend_from_slice(call),
            End::Return(es)=>{
                body.extend(es.iter().enumerate().map(|(i,e)|set(format!("r{i}"),e.clone())));
                body.extend_from_slice(returned);
            }
            End::Jump(j)=>body.extend(child(*j)?),
            End::Branch(e,a,b)=>body.push(S::If(e.clone(),child(*a)?,child(*b)?)),
            End::Switch(e,arms,d)=>body.push(S::Switch(e.clone(),arms.iter().map(|(key,j)|Some((*key,child(*j)?))).collect::<Option<_>>()?,match d {Some(j)=>Some(child(*j)?),None=>None})),
        }
        Some(body)
    }
    let mut down=path(m,m.entries[name],&[],&[S::Break],BTreeSet::new())?;
    let mut up=path(m,*next,&[S::Break],&[],BTreeSet::new())?;
    for body in [&mut down,&mut up] { paths::factor(body);borrowed_cells(body); }
    // Each phase has two exits. One ends its loop; the other falls through to
    // one shared boundary operation, retaining the original continuation join.
    down.extend(call(false));
    up.extend(returned(true));
    let mut cycle=vec![S::Loop(down)];
    cycle.extend(returned(false));
    cycle.push(S::Loop(up));
    cycle.extend(call(true));
    Some(vec![let_("frames",Ty::FrameRecords(k),p("native_records",vec![shape()])),S::Loop(cycle)])
}

fn unsigned(e: &E, known: &BTreeSet<String>) -> bool {
    match e {
        E::Int(n,_) => (0..=u32::MAX as i64).contains(n),
        E::V(n) => known.contains(n),
        E::Cast(_,Ty::U32 | Ty::U16 | Ty::U8) | E::Bool(_) => true,
        E::Cast(e,Ty::I64 | Ty::U64) => unsigned(e,known),
        E::Call { f,args,.. } if f=="wrap56" => unsigned(&args[0],known),
        E::Call { f,.. } if matches!(f.as_str(),"native_get32" | "native_get_fixed32") => true,
        E::Bin(Bop::And,a,b) => unsigned(a,known) || unsigned(b,known),
        E::Bin(Bop::Or | Bop::Xor,a,b) => unsigned(a,known) && unsigned(b,known),
        E::Bin(Bop::Lt | Bop::Le | Bop::Gt | Bop::Ge | Bop::Eq | Bop::Ne,_,_) => true,
        E::Bin(_,a,_) if matches!(&**a,E::Cast(_,Ty::U32) | E::Int(_,Ty::U32)) => true,
        E::Bin(Bop::Shr,a,b) if matches!(&**b,E::Int(n,_) if *n>=0) => unsigned(a,known),
        _ => false,
    }
}
fn proven(body: &[S], initial: &BTreeSet<String>) -> BTreeSet<String> {
    let mut defs:BTreeMap<String,Vec<E>>=BTreeMap::new();
    walk_stmts(body, &mut |s| {
        if let S::Let(Pat::One(n), _, e) | S::Set(n, e) = s { defs.entry(n.clone()).or_default().push(e.clone()); }
    });
    let mut known=initial.clone();
    loop {
        let before=known.len();
        for (n,es) in &defs { if es.iter().all(|e| unsigned(e,&known)) { known.insert(n.clone()); } }
        if known.len()==before { return known; }
    }
}
pub(super) fn compact(body: &mut [S], narrow: &BTreeSet<String>) {
    let narrow=proven(body,narrow);
    apply(body,&narrow);
}
fn apply(body: &mut [S], narrow: &BTreeSet<String>) {
    mutate_stmts(body, &mut |s| {
        // Storage narrowing only applies inside native control. Suspension
        // handlers and nested functions retain their representation and scope.
        if matches!(s, S::Try(..) | S::Res(..)) { return false; }
        if let Some(e) = s.parts_mut().0 {
            e.rewrite(&mut |e| if matches!(e, E::V(n) if narrow.contains(n)) { *e = cast(e.clone(), Ty::I64); });
        }
        match s {
            S::Let(Pat::One(n), ty, e) if narrow.contains(n) => { *ty = Ty::U32; *e = cast(e.clone(), Ty::U32); }
            S::Set(n, e) if narrow.contains(n) => *e = cast(e.clone(), Ty::U32),
            S::Decl(n, ty) if narrow.contains(n) => *ty = Ty::U32,
            _ => {}
        }
        true
    });
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn unsigned_temporaries_need_lossless_bounds_on_every_assignment() {
        let body=vec![let_("x",Ty::I64,bin(Bop::And,v("wide"),i64_(4294967295))),
            let_("y",Ty::I64,bin(Bop::Or,v("x"),v("word"))),
            let_("negative",Ty::I64,bin(Bop::Sub,v("word"),i64_(1))),
            let_("overflow",Ty::I64,bin(Bop::Add,v("word"),i64_(1))),
            let_("wrapping",Ty::I64,cast(bin(Bop::Add,cast(v("word"),Ty::U32),u32_(1)),Ty::I64)),
            let_("changes",Ty::I64,i64_(0)),
            S::If(E::Bool(true),vec![set("changes",i64_(-1))],vec![])];
        let known=proven(&body,&BTreeSet::from(["word".to_string()]));
        for n in ["x","y","wrapping"] { assert!(known.contains(n),"{n}"); }
        for n in ["negative","overflow","changes","wide"] { assert!(!known.contains(n),"{n}"); }
    }
    #[test] fn narrow_storage_keeps_original_operation_widths() {
        let mut body=vec![let_("x",Ty::I64,bin(Bop::And,v("wide"),i64_(4294967295))),
            ret(bin(Bop::Add,v("x"),i64_(1)))];
        compact(&mut body,&BTreeSet::new());
        assert_eq!(body[0],let_("x",Ty::U32,cast(bin(Bop::And,v("wide"),i64_(4294967295)),Ty::U32)));
        assert_eq!(body[1],ret(bin(Bop::Add,cast(v("x"),Ty::I64),i64_(1))));
    }
}

#[cfg(test)]
#[path = "../tests/support/native_storage.rs"]
mod storage_contract_tests;
