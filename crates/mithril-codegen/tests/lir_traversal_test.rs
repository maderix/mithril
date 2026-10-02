use mithril_codegen::lir::*;

fn nested(body: Vec<S>) -> S {
    S::Fn(Box::new(FnDef { name: "nested".into(), ctx: false, params: vec![], ret: Ty::I64, body, inline: Inline::Default, cold: false }))
}
fn controls() -> Vec<S> {
    vec![S::If(v("if"), vec![do_(v("yes"))], vec![do_(v("no"))]),
        S::Switch(v("switch"), vec![(1,vec![do_(v("arm"))])], Some(vec![do_(v("default"))])),
        S::Loop(vec![S::Machine(vec![(0,vec![S::Jump(v("jump")), S::Continue])])]),
        S::Try(Pat::One("value".into()),v("try"),"err".into(),vec![do_(v("handler"))]),
        S::Res(v("res"),"ok".into(),vec![ret(v("okbody"))],"err".into(),vec![ret(v("errbody"))]),
        nested(vec![do_(v("private"))])]
}
#[test]
fn postorder_expression_rewrite_visits_original_children_once_left_to_right() {
    let mut e=p("outer",vec![E::Arr(vec![E::Not(Box::new(v("a"))), E::Neg(Box::new(v("b")))]),
        bin(Bop::Add,cast(idx(E::Slice(vec![v("c")]),0),Ty::I64),
            E::Tup(vec![ok(v("d")),err(v("e"))]))]);
    let mut preorder=Vec::new();
    e.walk(&mut |e| match e { E::V(n)=>preorder.push(n.clone()), E::Call{f,..}=>preorder.push(f.clone()), _=>{} });
    assert_eq!(preorder,["outer","a","b","c","d","e"]);
    let mut order=Vec::new();
    e.rewrite(&mut |e| match e {
        E::V(n)=> { order.push(n.clone()); *e=cast(v(format!("new_{n}")),Ty::I64); },
        E::Call{f,..}=>order.push(f.clone()), _=>{}
    });
    assert_eq!(order, ["a","b","c","d","e","outer"]);
    let mut casts=0;
    e.walk(&mut |e| if matches!(e,E::Cast(_,Ty::I64)) { casts+=1; });
    assert_eq!(casts,6,"newly inserted casts must not be revisited");
}
#[test]
fn statement_parts_preserve_all_control_order_and_nested_function_scope() {
    let mut body=controls(); let before=body.last().unwrap().clone(); let mut order=Vec::new();
    retain_rewrite(&mut body,&mut |_|true,&mut |e| if let E::V(n)=e { order.push(n.clone()); });
    assert_eq!(order,["if","yes","no","switch","arm","default","jump","try","handler","res","okbody","errbody"]);
    assert_eq!(body.last(),Some(&before));
    let mut readonly=Vec::new();
    walk_stmts(&body,&mut |s| if let Some(E::V(n))=s.parts().0 { readonly.push(n.clone()); });
    assert_eq!(readonly,order);
}
#[test]
fn retain_precedes_expression_visits_for_each_block_and_dropped_subtrees() {
    let mut body=vec![do_(v("drop")),S::If(v("if"),vec![do_(v("drop")),do_(v("keep"))],vec![]),
        S::Loop(vec![do_(v("never"))]), nested(vec![do_(v("private"))])];
    let mut events=Vec::new();
    let mut expressions=Vec::new();
    retain_rewrite(&mut body,&mut |s| {
        let keep=!matches!(s,S::Do(E::V(n)) if n=="drop")&&!matches!(s,S::Loop(_));
        events.push(keep); keep
    },&mut |e| if let E::V(n)=e { expressions.push(n.clone()); });
    assert_eq!(events,[false,true,false,true,false,true]);
    assert_eq!(expressions,["if","keep"]);
    assert_eq!(body.len(),2);
}
#[test]
fn mutable_statement_preorder_uses_explicit_cfg_barriers() {
    let mut body=controls(); let saved_loop=body[2].clone(); let saved_fn=body[5].clone();
    let mut seen=Vec::new();
    mutate_stmts(&mut body,&mut |s| {
        if matches!(s,S::Loop(_)|S::Try(..)|S::Res(..)) { return false; }
        if let Some(E::V(n))=s.parts().0 { seen.push(n.clone()); }
        if let S::Do(e)=s { *e=bin(Bop::Add,e.clone(),i64_(1)); }
        true
    });
    assert_eq!(seen,["if","yes","no","switch","arm","default"]);
    assert_eq!(body[2],saved_loop); assert_eq!(body[5],saved_fn);
    let S::If(_,yes,no)=&body[0] else { panic!() };
    assert_eq!(yes,&vec![do_(bin(Bop::Add,v("yes"),i64_(1)))]);
    assert_eq!(no,&vec![do_(bin(Bop::Add,v("no"),i64_(1)))]);
}
