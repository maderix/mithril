//! Share a tail caller's integer prefix; its two entries keep their own
//! return protocol and charges. Unsupported continuations keep both bodies.
use super::*;
use crate::scalar::{Kind, PTy, Sig};
use mithril_front::core::CoreModule;

fn charge(s: &S) -> bool {
    matches!(s, S::Do(E::Call { f, ctx: false, args }) if f == "work_fuel" && *args == vec![v("fuel"), v("fl")])
}
/// An int between its port and native forms; an owned port argument may
/// be a box, released after the read.
fn integer_port(e: E, pack: bool) -> E {
    if pack { num(e) } else { p("take_i", vec![e]) }
}
fn returned(ret_ty: Ty, values: Vec<E>, ports: bool) -> E {
    let value = match ret_ty {
        Ty::I64 => values.into_iter().next().unwrap(),
        _ if ports => E::Arr(values),
        _ => E::Tup(values),
    };
    if ports { ok(value) } else { value }
}
fn identity(rest: &[S], outs: &[String], ret_ty: Ty) -> bool {
    let mut env = Env::new();
    let mut charged = false;
    for (i, s) in rest.iter().enumerate() {
        match s {
            S::Let(Pat::One(n), Ty::Infer, e) if matches!(e, E::Tup(_) | E::V(_)) => {
                let e = e.substitute(&env, false).unwrap();
                env.insert(n.clone(), e);
            }
            S::Do(_) if charge(s) && !charged => charged = true,
            S::Ret(e) if i + 1 == rest.len() && charged => {
                return e.substitute(&env, false).unwrap()
                    == if ret_ty == Ty::I64 {
                        v(&outs[0])
                    } else {
                        E::Tup(outs.iter().map(v).collect())
                    }
            }
            _ => return false,
        }
    }
    false
}
fn plan(
    body: &[S],
    ret_ty: Ty,
    payload: usize,
    sigs: &[Option<Sig>],
    routes: &mut BTreeMap<u64, (u32, usize)>,
) -> Option<Vec<S>> {
    let pack = |tag, mut values: Vec<E>| {
        values.resize(payload, i64_(0));
        ret(E::Tup([vec![i64_(tag), v("fl")], values].concat()))
    };
    let mut out = Vec::new();
    let mut tuples = Env::new();
    for (i, s) in body.iter().enumerate() {
        match s {
            S::Let(pat, _, E::Call { f, ctx: false, args }) if f.starts_with("s_") => {
                let fid: u32 = f[2..].parse().ok()?;
                let sig = sigs.get(fid as usize)?.as_ref()?;
                let callee_ret = match sig.ret {
                    Kind::S1 => Ty::I64,
                    Kind::SK(k) => Ty::Tup(k),
                    Kind::No => return None,
                };
                let outs = pat_names(pat);
                if sig.params.iter().any(|p| *p != PTy::I)
                    || callee_ret != ret_ty
                    || outs.len() != width(ret_ty)
                    || args.len() != sig.params.len() + 1
                    || args.first() != Some(&v("fuel"))
                    || args[1..].iter().any(|e| !operations::Policy::Prefix.accepts(e))
                    || !identity(&body[i + 1..], &outs, ret_ty)
                {
                    return None;
                }
                routes.insert(1 + fid as u64, (fid, args.len() - 1));
                out.push(pack(1 + fid as i64, args[1..].to_vec()));
                return Some(out);
            }
            S::Let(Pat::One(n), _, e) if operations::Policy::Prefix.accepts(e) && n != "fuel" => {
                if matches!(e, E::Tup(_)) {
                    tuples.insert(n.clone(), e.clone());
                }
                out.push(s.clone());
            }
            S::If(e, a, b) if operations::Policy::Prefix.accepts(e) && i + 1 == body.len() => {
                out.push(S::If(
                    e.clone(),
                    plan(a, ret_ty, payload, sigs, routes)?,
                    plan(b, ret_ty, payload, sigs, routes)?,
                ));
                return Some(out);
            }
            S::Do(_) if charge(s) && matches!(body.get(i + 1), Some(S::Ret(_))) => {}
            // a tail call settles this frame's work, then returns the call itself
            S::Ret(E::Call { f, ctx: false, args }) if f.starts_with("s_") && i + 1 == body.len() && i > 0 && charge(&body[i - 1]) => {
                let fid: u32 = f[2..].parse().ok()?;
                let sig = sigs.get(fid as usize)?.as_ref()?;
                let callee_ret = match sig.ret {
                    Kind::S1 => Ty::I64,
                    Kind::SK(k) => Ty::Tup(k),
                    Kind::No => return None,
                };
                if sig.params.iter().any(|p| *p != PTy::I)
                    || callee_ret != ret_ty
                    || args.len() != sig.params.len() + 1
                    || args.first() != Some(&v("fuel"))
                    || args[1..].iter().any(|e| !operations::Policy::Prefix.accepts(e))
                {
                    return None;
                }
                routes.insert(1 + fid as u64, (fid, args.len() - 1));
                out.push(pack(1 + fid as i64, args[1..].to_vec()));
                return Some(out);
            }
            S::Ret(e) if i + 1 == body.len() && i > 0 && charge(&body[i - 1]) && operations::Policy::Prefix.accepts(e) => {
                let e = match e {
                    E::V(n) => tuples.get(n).unwrap_or(e).clone(),
                    _ => e.clone(),
                };
                let values = match (ret_ty, e) {
                    (Ty::I64, e) => vec![e],
                    (Ty::Tup(k), E::Tup(xs)) if xs.len() == k => xs,
                    _ => return None,
                };
                out.push(pack(0, values));
                return Some(out);
            }
            _ => return None,
        }
    }
    None
}
/// Replace only a certified tail caller. The growth entry's burn and
/// fuel-out continuation remain byte-for-byte the original LIR statements.
pub(super) fn share(
    native: &mut FnDef,
    growth: &mut FnDef,
    module: &CoreModule,
    sigs: &[Option<Sig>],
) -> Option<FnDef> {
    if width(native.ret) == 0
        || growth.ret
            != if native.ret == Ty::I64 {
                Ty::Res
            } else {
                Ty::ResArr(width(native.ret))
            }
        || native.ctx
        || native.params.first() != Some(&("fuel".into(), Ty::RefI64))
        || !matches!(native.body.first(), Some(S::Let(Pat::One(n), Ty::I64, E::Int(1, Ty::I64))) if n == "fl")
        || growth.body.len() < 3
        || growth.body[0] != do_(p("stack_guard", vec![]))
        || growth.body[1] != burn_fuel()
        || !matches!(&growth.body[2], S::If(e, _, _) if *e == bin(Bop::Lt, E::Deref("fuel".into()), i64_(0)))
    {
        return None;
    }
    let mut payload = width(native.ret);
    walk_exprs(&native.body, &mut |e| {
        if let E::Call { f, args, .. } = e {
            if f.starts_with("s_") { payload = payload.max(args.len().saturating_sub(1)); }
        }
    });
    let mut routes = BTreeMap::new();
    let body = plan(&native.body, native.ret, payload, sigs, &mut routes)?;
    let mut protocols: BTreeMap<u32, BTreeSet<String>> = BTreeMap::new();
    let prefixes: &[&str] = if native.ret == Ty::I64 { &["d", "q"] } else { &["n"] };
    let forms: BTreeMap<_, _> = routes.values().flat_map(|(g, arity)| {
        prefixes.iter().map(move |prefix| (format!("{prefix}_{g}"), (*g, *arity)))
    }).collect();
    walk_exprs(&growth.body[3..], &mut |e| {
        if let E::Call { f, ctx: true, args } = e {
            if let Some((g, arity)) = forms.get(f) {
                if args.len() == arity + 1 && args.first() == Some(&v("fuel")) {
                    protocols.entry(*g).or_default().insert(f.clone());
                }
            }
        }
    });
    if routes.is_empty()
        || routes
            .values()
            .any(|(g, _)| protocols.get(g).is_none_or(|ps| ps.len() != 1))
    {
        return None;
    }
    let allowed: BTreeSet<_> = protocols.values().flat_map(|ps| ps.iter().cloned()).collect();
    let mut valid = true;
    let params: BTreeSet<String> = growth.params.iter().map(|(n, _)| n.clone()).collect();
    walk_stmts(&growth.body[3..], &mut |s| {
        // releasing an owned int parameter on a path that does not use it is
        // storage, as take_i is: the prefix releases every argument at entry
        let release = matches!(s, S::Do(E::Call { f, args, .. }) if f == "free_val" && matches!(args.as_slice(), [E::V(n)] if params.contains(n)));
        if release {
            return;
        }
        valid &= matches!(
            s,
            S::Let(Pat::One(_), _, _) | S::If(..) | S::Ret(_) | S::Unreachable
        );
        if let Some(e) = s.parts().0 {
            let mut e = e.clone();
            e.rewrite(&mut |e| match e {
                E::Call { f, ctx, args }
                    if *ctx && allowed.contains(f) && args.first() == Some(&v("fuel")) =>
                {
                    *ctx = false;
                    *f = "wrap56".into();
                    args.remove(0);
                }
                // releasing an owned int is storage, not meaning: the shared
                // prefix reads its arguments with take_i once at entry
                E::Call { f, ctx: false, args } if f == "take_i" && args.len() == 1 => *f = "as_i".into(),
                E::Ok(x) => *e = (**x).clone(),
                E::Arr(xs) => *e = E::Tup(xs.clone()),
                _ => {}
            });
            valid &= operations::Policy::Prefix.accepts(&e);
        }
    });
    if !valid {
        return None;
    }
    let mut prefix = native.clone();
    prefix.name = format!("prefix_{}", native.name);
    prefix.params.remove(0);
    prefix.ret = Ty::Tup(payload + 2);
    prefix.body = body;
    let names: Vec<_> = [
        vec!["route".into(), "work".into()],
        (0..payload).map(|i| format!("p{i}")).collect(),
    ]
    .concat();
    let fid: u32 = native.name[2..].parse().ok()?;
    let convert = |e, _target: u32, ports, pack| if ports { integer_port(e, pack) } else { e };
    let prepare = |ports| S::Let(Pat::Tup(names.clone()), Ty::Infer,
        p(&prefix.name, native.params[1..].iter().map(|(n, _)| convert(v(n), fid, ports, false)).collect()));
    let dispatch = |ports| {
        // Both entries dispatch the same routes; this adapter preserves their
        // distinct value representation and work-charge protocol.
        let finish = |value, call| {
            if ports { return vec![ret(value)]; }
            let mut body = Vec::new();
            let value = if call { body.push(let_("value", native.ret, value)); v("value") } else { value };
            body.extend([do_(p("work_fuel", vec![v("fuel"), v("work")])), ret(value)]);
            body
        };
        let values = (0..width(native.ret)).map(|i| convert(v(format!("p{i}")), fid, ports, true)).collect();
        let mut arms = vec![(0, finish(returned(native.ret, values, ports), false))];
        for (tag, (g, arity)) in &routes {
            let args = std::iter::once(v("fuel"))
                .chain((0..*arity).map(|i| convert(v(format!("p{i}")), *g, ports, true)))
                .collect();
            let call = E::Call {
                f: if ports {
                    protocols[g].first().unwrap().clone()
                } else {
                    format!("s_{g}")
                },
                ctx: ports,
                args,
            };
            arms.push((*tag, finish(call, true)));
        }
        // The private prefix constructs only zero and these recorded tags.
        // Its final legal route is therefore an ordinary returning default.
        let (_, default) = arms.pop().unwrap();
        S::Switch(v("route"), arms, Some(default))
    };
    let native_body = vec![prepare(false), dispatch(false)];
    let mut growth_body = growth.body[..3].to_vec();
    growth_body.extend([prepare(true), dispatch(true)]);
    // Shapes and signature checks above certify every dispatch target.
    debug_assert!(routes.values().all(|(g, _)| (*g as usize) < module.fns.len()));
    native.body = native_body;
    growth.body = growth_body;
    Some(prefix)
}

#[cfg(test)]
#[path = "../tests/support/native_prefix.rs"]
mod tests;
