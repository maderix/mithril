//! mithril-codegen: dual-mode Rust emission.
//!
//! `emit_rust(m)` turns a `CoreModule` that `mithril_net::specialize` has
//! already reduced (every static redex fired; see docs/design.md 3b) into a
//! single `main.rs` that depends only on `mithril_rt` (whose `prelude`
//! holds the program-independent helpers) and implements its [`Program`]
//! protocol:
//!
//! * **dive form** (`d_<fid>`) — Core compiled to sequential Rust. Values are
//!   Port raws (`u64`): i56 immediates inline (`Tag::Num`), constructors as
//!   cells (arity <= 2 direct, wider ctors chained), floats boxed in a cell.
//!   Self-tail-recursive functions become loops. Fuel is decremented once per
//!   call / loop iteration; on exhaustion at a point whose residue is exactly
//!   "the pending call" (function entry / loop top with a known destination)
//!   the pending call is spawned as a redex and the dive reports
//!   `Suspended`; a fuel-out deep inside a nested dive unwinds (`Err(false)`)
//!   and the top level falls back to the rule form.
//! * **native scalar form** (`s_<fid>`, scalar.rs) — functions over ints and
//!   int arrays as plain Rust, the fastest region of the same net.
//! * **rule form** (`fc_<fid>` fire arm + `x_<fid>` expansion + `sg_<k>`
//!   continuation segments) — the body is ANF-normalized so every call is a
//!   let-RHS or a tail call; fork points become waiting records (two
//!   independent adjacent calls share one pend-2 record, exactly like the
//!   bitonic spike's bsort), continuations become numbered segment rules with
//!   any extra environment spilled to a cell chain.
//! * **proven folds** (`CoreFn.fold`: WrapAdd / WrapAdd32 / TupleWrapAdd /
//!   TupleWrapAdd32) — the fold helper's CALL rule splits large ranges into a
//!   chunked binary fork (records + spawned redexes, never OS threads): leaf
//!   chunks run the dive form under fuel, the join rule is the combiner
//!   (wrapping add mod 2^56, or masked to the low 32 bits for the *32
//!   variants; elementwise over tuple accumulators).
//!
//! If specialization reduced `main` to a value, the program is const-folded:
//! the emitted `main.rs` just prints it.
//!
//! Rule numbering: 0 = boot (fires `main` with parent `ROOT`), `1 + fid` =
//! CALL rules, then fold join rules, then segments. Generated `main` takes
//! `argv[1]` = threads (default 1) and `argv[2]` = fuel (default 4096).
//!
//! Memory: values are consumed linearly (seq.rs), cells are freed on their
//! last use; refcounts only where linearity cannot be proven.

mod fast;
mod fold;
mod range;
mod rewrite;
mod rules;
mod scalar;
mod seq;
mod ty;

use mithril_front::core::{Core, CoreModule, Val};
use std::collections::BTreeSet;

/// Canonical printing of an `eval_core` value; the generated program's
/// `show` produces exactly this for the corresponding runtime value.
/// A Core expression that is a literal value.
fn core_value(e: &Core) -> Option<mithril_front::Val> {
    use mithril_front::Val;
    Some(match e {
        Core::Num(n) => Val::I(*n),
        Core::Flo(f) => Val::F(*f),
        Core::Ctor(c, xs) => Val::C(*c, xs.iter().map(core_value).collect::<Option<Vec<_>>>()?),
        Core::Tuple(xs) => Val::T(xs.iter().map(core_value).collect::<Option<Vec<_>>>()?),
        _ => return None,
    })
}

pub fn fmt_val(v: &Val) -> String {
    match v {
        Val::I(i) => i.to_string(),
        Val::F(x) => format!("{:?}", x),
        Val::C(c, fs) => {
            format!("C{}({})", c, fs.iter().map(fmt_val).collect::<Vec<_>>().join(", "))
        }
        Val::T(fs) => format!("({})", fs.iter().map(fmt_val).collect::<Vec<_>>().join(", ")),
        Val::A(fs) => format!("[{}]", fs.iter().map(fmt_val).collect::<Vec<_>>().join(", ")),
        Val::L(..) => "<closure>".to_string(),
    }
}

/// Unary constructors whose field is a proven-i56 at EVERY construction
/// site are unboxed: the value rides in the port (tag = TU_BASE + slot, the
/// i56 in the payload), so building/matching/freeing them costs no cell.
/// Returns ctor id -> unbox slot. General, type-shape-driven; a single
/// non-int construction site disqualifies the ctor everywhere.
pub(crate) fn unboxed_ctors(
    m: &CoreModule,
    tys: &ty::Types,
) -> std::collections::HashMap<u32, u8> {
    let mut slot = 0u8;
    let mut out = std::collections::HashMap::new();
    for c in 0..m.ctors.len() as u32 {
        if m.ctors[c as usize].1 == 1 && tys.field[c as usize][0] == ty::Ty::Int {
            assert!(slot < 200, "too many unboxed ctor tags");
            out.insert(c, slot);
            slot += 1;
        }
    }
    out
}
/// Vars (params + locals) of `fid` proven Int by inference.
pub(crate) fn ints_of(tys: &ty::Types, fid: usize) -> std::collections::HashSet<u32> {
    tys.locals[fid]
        .iter()
        .enumerate()
        .filter(|(_, t)| **t == ty::Ty::Int)
        .map(|(i, _)| i as u32)
        .collect()
}

/// Emit a complete `main.rs` for the module (see module docs).
/// Alpha-rename every binder (Let, match field) of every function to a
/// unique id, so one var id means one value everywhere in the body (the
/// desugarer reuses ids across match arms). Parameters keep 0..arity.
fn uniquify(m: &CoreModule) -> CoreModule {
    fn go(e: &Core, env: &mut std::collections::HashMap<u32, u32>, next: &mut u32) -> Core {
        match e {
            Core::Var(i) => Core::Var(*env.get(i).unwrap_or(i)),
            Core::Num(_) | Core::Flo(_) => e.clone(),
            Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(go(a, env, next)), Box::new(go(b, env, next))),
            Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(go(a, env, next)), Box::new(go(b, env, next))),
            Core::If(c, t, f) => Core::If(Box::new(go(c, env, next)), Box::new(go(t, env, next)), Box::new(go(f, env, next))),
            Core::Lam(x, b) => {
                let nx = *next;
                *next += 1;
                let saved = env.insert(*x, nx);
                let b2 = go(b, env, next);
                match saved {
                    Some(v) => env.insert(*x, v),
                    None => env.remove(x),
                };
                Core::Lam(nx, Box::new(b2))
            }
            Core::App(f, a) => Core::App(Box::new(go(f, env, next)), Box::new(go(a, env, next))),
            Core::Let(x, r, b) => {
                let r2 = go(r, env, next);
                let nx = *next;
                *next += 1;
                let saved = env.insert(*x, nx);
                let b2 = go(b, env, next);
                match saved {
                    Some(v) => env.insert(*x, v),
                    None => env.remove(x),
                };
                Core::Let(nx, Box::new(r2), Box::new(b2))
            }
            Core::Call(g, a) => Core::Call(*g, a.iter().map(|x| go(x, env, next)).collect()),
            Core::Ctor(c, a) => Core::Ctor(*c, a.iter().map(|x| go(x, env, next)).collect()),
            Core::Reuse(v, c, a) => Core::Reuse(*env.get(v).unwrap_or(v), *c, a.iter().map(|x| go(x, env, next)).collect()),
            Core::Tuple(a) => Core::Tuple(a.iter().map(|x| go(x, env, next)).collect()),
            Core::Prim(p, a) => Core::Prim(*p, a.iter().map(|x| go(x, env, next)).collect()),
            Core::Proj(b, i) => Core::Proj(Box::new(go(b, env, next)), *i),
            Core::Match(sc, arms) => {
                let sc2 = go(sc, env, next);
                let arms2 = arms
                    .iter()
                    .map(|(c, bs, body)| {
                        let mut saved = Vec::new();
                        let nbs: Vec<u32> = bs
                            .iter()
                            .map(|b| {
                                let nb = *next;
                                *next += 1;
                                saved.push((*b, env.insert(*b, nb)));
                                nb
                            })
                            .collect();
                        let body2 = go(body, env, next);
                        for (b, old) in saved.into_iter().rev() {
                            match old {
                                Some(v) => env.insert(b, v),
                                None => env.remove(&b),
                            };
                        }
                        (*c, nbs, body2)
                    })
                    .collect();
                Core::Match(Box::new(sc2), arms2)
            }
        }
    }
    let mut out = m.clone();
    for f in out.fns.iter_mut() {
        let mut env = std::collections::HashMap::new();
        let mut next = f.arity as u32;
        f.body = go(&f.body, &mut env, &mut next);
    }
    out
}

/// Native int representation override (tests): `None` = chosen per
/// function by cost, `Some(false)` = all plain, `Some(true)` = all
/// pre-shifted. Both representations must compute identical results.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EmitOpts {
    pub int_rep: Option<bool>,
}

/// Emit the Rust program of a module that has been specialized by the
/// interaction rules (`mithril_net::specialize`): the residual program.
pub fn emit_rust(m: &CoreModule) -> String {
    emit_rust_opts(m, EmitOpts::default())
}

pub fn emit_rust_opts(m: &CoreModule, opts: EmitOpts) -> String {
    // the passes recurse along let chains, which compile-time unfolding
    // makes long: run on a stack sized for that, not the caller's
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn_scoped(s, move || {
                scalar::FORCE_REP.with(|f| f.set(opts.int_rep));
                emit_rust_inner(m)
            })
            .expect("spawn codegen thread")
            .join()
            .unwrap_or_else(|p| std::panic::resume_unwind(p))
    })
}

fn emit_rust_inner(m: &CoreModule) -> String {
    // Constant folding, inlining, branch selection and static evaluation
    // happened in the net (specialize); what remains here are the shapes
    // codegen itself owns: mutual tail recursion into loops, and
    // if-conversion of loop back-edges.
    let m_s = rewrite::tail_inline(m);
    if let Some(which) = std::env::var_os("MITHRIL_DUMP_CORE") {
        let which = which.to_string_lossy().to_string();
        for (fid, f) in m_s.fns.iter().enumerate() {
            if f.name == which || which == "all" {
                eprintln!("core {fid} {} = {:?}", f.name, f.body);
            }
        }
    }
    let m_u = rewrite::if_convert(&uniquify(&m_s));
    // records_to_tuples (rewrite.rs) is parked: without native multi-value
    // returns in the dive form it only trades ctor cells for tuple chains.
    let m = &m_u;
    // Const path: the whole program reduced to its value at compile time.
    if let Some(v) = core_value(&m.fns[m.main as usize].body) {
        return format!(
            "// GENERATED by mithril-codegen (net quiesced at compile time).\nfn main() {{\n    println!(\"{{}}\", {:?});\n}}\n",
            fmt_val(&v)
        );
    }

    let nf = m.fns.len();
    let folds: Vec<Option<fold::ParFold>> = (0..nf).map(|f| fold::par_fold(m, f as u32)).collect();
    let mut join_rule: Vec<u16> = vec![0; nf];
    let mut next: u16 = (1 + nf) as u16;
    for f in 0..nf {
        if folds[f].is_some() {
            join_rule[f] = next;
            next += 1;
        }
    }
    let mut sq = rules::SegQ { next, q: Vec::new(), memo: Default::default(), holes: Vec::new() };
    // forwarding segment: fuel-out at a dive entry spawns the pending call
    // against a record of this rule, which just passes the value upward
    let fwd = sq.add(u32::MAX, vec![0], vec![], Core::Var(0));

    // Normalize every body first, then infer per-parameter borrow modes
    // (read-only params are lent, not consumed; see `borrows`).
    let bodies: Vec<Core> = m
        .fns
        .iter()
        .map(|f| {
            let mut c = max_var(&f.body).max(f.arity as u32) + 1;
            rules::normalize(&f.body, &mut c)
        })
        .collect();

    // types of the bodies the emitters see: ANF introduces fresh vars (call
    // results, intermediate values) that must carry types too
    let tys = {
        let mut mn = m.clone();
        for (f, b) in mn.fns.iter_mut().zip(&bodies) {
            f.body = b.clone();
        }
        ty::infer(&mn)
    };
    let unbox = unboxed_ctors(m, &tys);
    let mut scal = scalar::classify(m, &tys);
    // Native scalar code never suspends, so it cannot split work: a
    // function that forks, and every function that (transitively) calls
    // one, runs in dive form instead (its leaves still call native code).
    {
        let calls: Vec<std::collections::HashSet<u32>> = m.fns.iter().map(|f| callees(&f.body)).collect();
        // parallel sources: forking recursion and proven folds (split
        // across workers by their CALL rule)
        let mut reach: Vec<bool> = (0..m.fns.len())
            .map(|f| scal[f].is_some() && (fork_recursive(f as u32, &m.fns[f]) || folds[f].is_some()))
            .collect();
        loop {
            let mut changed = false;
            for f in 0..m.fns.len() {
                if !reach[f] && calls[f].iter().any(|g| reach[*g as usize]) {
                    reach[f] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (f, r) in reach.iter().enumerate() {
            if *r {
                scal[f] = None;
            }
        }
    }
    let native: Vec<bool> = scal.iter().map(|s| s.is_some()).collect();
    {
        let sn: Vec<Option<scalar::Sig>> = scal.iter().zip(&native).map(|(s, n)| if *n { s.clone() } else { None }).collect();
        let reps = scalar::choose_reps(m, &sn);
        if std::env::var_os("MITHRIL_DEBUG_SCALAR").is_some() {
            for (f, r) in reps.iter().enumerate() {
                if native[f] {
                    eprintln!("rep {f} {}: {}", m.fns[f].name, if *r { "shifted" } else { "plain" });
                }
            }
        }
        scalar::SHIFTED.with(|l| *l.borrow_mut() = reps);
        let cx = scalar::needs_ctx(m, &sn);
        scalar::CTX.with(|c| *c.borrow_mut() = cx);
    }
    scalar::LEAF.with(|l| {
        *l.borrow_mut() = (0..m.fns.len()).map(|f| native[f] && !any_call(&m.fns[f].body)).collect()
    });
    BOUNDED.with(|b| *b.borrow_mut() = bounded_fns(m).iter().zip(&native).map(|(x, y)| *x || *y).collect());
    scalar::SIGS.with(|s| {
        *s.borrow_mut() = scal.iter().zip(&native).map(|(sig, n)| if *n { sig.clone() } else { None }).collect()
    });
    let (bor, bsets) = borrows(m, &bodies, &tys, &unbox);
    if std::env::var_os("MITHRIL_DEBUG_TY").is_some() {
        for (fid, f) in m.fns.iter().enumerate() {
            eprintln!("bor {fid} {} {:?}", f.name, bor[fid]);
        }
    }
    let iret: Vec<bool> = tys.ret.iter().map(|t| *t == ty::Ty::Int).collect();
    if std::env::var_os("MITHRIL_DEBUG_TY").is_some() {
        for (fid, f) in m.fns.iter().enumerate() {
            let ps: Vec<String> = (0..f.arity as u32).map(|p| format!("{:?}", tys.var(fid, p))).collect();
            eprintln!("ty {fid} {}({}) -> {:?}  locals {:?}", f.name, ps.join(", "), tys.ret[fid], tys.locals[fid]);
        }
    }
    // static reuse rewrite: consumed same-arity cells are rebuilt in place
    let bodies: Vec<Core> = bodies.iter().map(|b| rewrite::mark_reuse(b, m, &unbox)).collect();
    let shared = std::cell::RefCell::new(seq::Shared::default());
    // fold splits deep-share the fold's extra args (see fold.rs)
    for (fid, pf) in folds.iter().enumerate() {
        if let Some(pf) = pf {
            for i in 2..m.fns[fid].arity {
                if i == pf.acc {
                    continue;
                }
                let mut sh = shared.borrow_mut();
                match tys.params[fid][i] {
                    ty::Ty::Adt(c) => {
                        sh.classes.insert(c);
                    }
                    ty::Ty::Tup(_) => sh.tuples = true,
                    ty::Ty::Dyn => sh.poison = true,
                    _ => {}
                }
            }
        }
    }
    // destination-passing callees (TRMC list builders with a tail param)
    let dps: Vec<Option<(usize, u32)>> = (0..nf)
        .map(|fid| {
            if bor[fid].iter().any(|b| *b) || scal[fid].is_some() {
                return None;
            }
            seq::dps_param(fid as u32, &mut |g| is_bounded(g), &bodies[fid], m.fns[fid].arity, m, &unbox)
        })
        .collect();
    seq::DPS.with(|d| *d.borrow_mut() = dps.clone());
    // base-case wrappers for recursive dive functions (see fast.rs)
    let fast_code: Vec<Option<String>> = (0..nf)
        .map(|fid| {
            if scal[fid].is_some() {
                return None;
            }
            fast::fast_fn(m, fid as u32, &bodies[fid], &tys, &unbox, &bor[fid])
        })
        .collect();
    fast::FAST.with(|f| *f.borrow_mut() = fast_code.iter().map(|c| c.is_some()).collect());
    // native multi-value returns: dive functions returning a k-tuple (see
    // seq::NTUP); not scalar (own lowering), not destination-passing, not
    // behind a base-case wrapper, not main
    let ntup: Vec<usize> = (0..nf)
        .map(|fid| match tys.ret[fid] {
            ty::Ty::Tup(k)
                if (2..=8).contains(&k)
                    && scal[fid].is_none()
                    && dps[fid].is_none()
                    && fast_code[fid].is_none()
                    && fid as u32 != m.main
                    && std::env::var_os("MITHRIL_NO_NTUP").is_none() =>
            {
                k as usize
            }
            _ => 0,
        })
        .collect();
    seq::NTUP.with(|n| *n.borrow_mut() = ntup);
    seq::FOLDS.with(|f| *f.borrow_mut() = folds.iter().map(|p| p.is_some()).collect());
    {
        // a native function's bridge is live when something may dive it:
        // the entry, a fold, or any caller that is not native
        let calls: Vec<std::collections::HashSet<u32>> = m.fns.iter().map(|f| callees(&f.body)).collect();
        let live: Vec<bool> = (0..nf)
            .map(|g| {
                g as u32 == m.main
                    || folds[g].is_some()
                    || (0..nf).any(|f| calls[f].contains(&(g as u32)) && scal[f].is_none())
                    || !(0..nf).any(|f| f != g && calls[f].contains(&(g as u32)))
            })
            .collect();
        scalar::BRIDGE_LIVE.with(|b| *b.borrow_mut() = live);
    }
    seq::FOLD_SPLIT.with(|f| {
        *f.borrow_mut() = folds
            .iter()
            .enumerate()
            .map(|(fid, p)| p.as_ref().map(|pf| fold::split_snippet_dive(fid as u32, m.fns[fid].arity, pf, join_rule[fid])))
            .collect()
    });
    let trace = std::env::var_os("MITHRIL_TRACE_GEN").is_some();
    let mut fns_code = String::new();
    for fid in 0..nf {
        if trace {
            eprintln!("gen fn {} ({}) segs={} code={}B", fid, m.fns[fid].name, sq.q.len(), fns_code.len());
        }
        if scal[fid].is_some() {
            // native scalar form + bridging dive form (see scalar.rs)
            fns_code.push_str(&scalar::scalar_fn(m, fid as u32, &scal, &bor, true));
        } else {
            fns_code.push_str(&seq::dive_fn(m, fid as u32, &bodies[fid], &bor, &bsets[fid], &mut sq, fwd, &unbox, &tys, &iret, &shared));
        }
        if let Some(q) = &fast_code[fid] {
            fns_code.push_str(q);
        }
        if let Some((p, c)) = dps[fid] {
            fns_code.push_str(&seq::dps_fn(m, fid as u32, p, c, &bodies[fid], &bor, &mut sq, &unbox, &tys, &iret, &shared));
        }
        fns_code.push_str(&rules::expand_fn(m, fid as u32, &bodies[fid], &bor, &mut sq, &unbox, &tys, &iret, &shared));
        fns_code.push_str(&call_fn(m, fid as u32, folds[fid].as_ref(), join_rule[fid], &bor));
        if let Some(pf) = &folds[fid] {
            fns_code.push_str(&fold::join_fn(fid as u32, pf));
            fns_code.push_str(&fold::est_static(fid as u32));
        }
    }
    // Segments may enqueue further segments while being emitted.
    let mut done = 0;
    while done < sq.q.len() {
        let seg = sq.q[done].clone();
        done += 1;
        if trace && done % 500 == 0 {
            eprintln!("gen seg {} of {} (fn {}) code={}B", done, sq.q.len(), seg.fid, fns_code.len());
        }
        fns_code.push_str(&rules::segment_fn(m, &seg, &bor, &mut sq, &unbox, &tys, &iret, &shared));
    }

    let n_rules = sq.next as usize;
    let mut fire_arms = String::new();
    fire_arms.push_str(&format!(
        "            0 => fc_{}(ctx, Redex {{ a: e.a, b: e.b, aux: ROOT }}),\n",
        m.main
    ));
    for fid in 0..nf {
        fire_arms.push_str(&format!("            {} => fc_{}(ctx, e),\n", 1 + fid, fid));
    }
    for fid in 0..nf {
        if folds[fid].is_some() {
            fire_arms.push_str(&format!("            {} => jn_{}(ctx, e),\n", join_rule[fid], fid));
        }
    }
    // rules whose firing may run a dive: CALL entries and segments with calls
    let mut diving: Vec<usize> = (1..=nf).collect();
    for seg in &sq.q {
        fire_arms.push_str(&format!("            {} => sg_{}(ctx, e),\n", seg.id, seg.id));
        if has_call(&seg.body) {
            diving.push(seg.id as usize);
        }
    }
    for (id, cid) in &sq.holes {
        fire_arms.push_str(&format!("            {id} => hl_{id}(ctx, e),\n"));
        fns_code.push_str(&format!(
            "fn hl_{id}(ctx: &mut Wctx, e: Redex) {{\nlet inf = ctx.rec(e.aux as u32);\nctx.set(inf.s, 1, e.a);\nctx.deliver(inf.parent, con(inf.d, {cid}u16, 2));\n}}\n\n"
        ));
    }
    let diving: String = diving.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ");
    let mut dive_arms = String::new();
    for fid in 0..nf {
        let ar = m.fns[fid].arity;
        let args: String = (0..ar).map(|i| format!(", args[{}]", i + 1)).collect();
        // the owned-argument entry: values lent to borrowed parameters are
        // released once the dive returns, finished or suspended (a
        // suspension took its own references to what it still reads)
        let lent: String = (0..ar)
            .filter(|&i| bor[fid][i])
            .map(|i| format!("free_val(ctx, args[{}]);\n", i + 1))
            .collect();
        if lent.is_empty() {
            dive_arms.push_str(&format!("            {fid} => d_{fid}(ctx, fuel{args}),\n"));
        } else {
            dive_arms.push_str(&format!("            {fid} => {{\nlet r = d_{fid}(ctx, fuel{args});\n{lent}r\n}}\n"));
        }
    }

    let mut out = String::with_capacity(fns_code.len() + 8192);
    out.push_str(PRELUDE);
    // slot -> original ctor id, for printing unboxed constructor values
    {
        let mut cids = vec![0u32; unbox.len()];
        for (cid, slot) in &unbox {
            cids[*slot as usize] = *cid;
        }
        out.push_str(&format!(
            "const UNBOX_CID: [u32; {}] = [{}];\n",
            cids.len(),
            cids.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")
        ));
    }
    // static linearity: close the shared set over field types; every ctor
    // of an unshared class is linear (refcount never read or written)
    {
        let mut sh = shared.borrow_mut();
        let mut changed = true;
        while changed {
            changed = false;
            let classes: Vec<u32> = sh.classes.iter().copied().collect();
            for c in 0..m.ctors.len() {
                if !classes.contains(&tys.class_of[c]) {
                    continue;
                }
                for ft in &tys.field[c] {
                    match *ft {
                        ty::Ty::Adt(d) => changed |= sh.classes.insert(d),
                        ty::Ty::Tup(_) => {
                            if !sh.tuples {
                                sh.tuples = true;
                                changed = true;
                            }
                        }
                        ty::Ty::Dyn => sh.poison = true,
                        _ => {}
                    }
                }
            }
        }
        let lin: Vec<bool> = (0..m.ctors.len())
            .map(|c| !sh.poison && !sh.classes.contains(&tys.class_of[c]))
            .collect();
        out.push_str(&format!(
            "const LIN: [bool; {}] = [{}];\nconst LIN_TUP: bool = {};\n",
            lin.len(),
            lin.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", "),
            !sh.poison && !sh.tuples
        ));
    }
    out.push_str(&fns_code);
    out.push_str(&format!(
        "struct Pg {{ fuel: u32 }}

impl Program for Pg {{
    fn n_rules(&self) -> usize {{ {n_rules} }}
    fn rule_cost(&self, rule: u16) -> u32 {{
        // An entry that may dive can burn a whole budget; pure joins are tiny.
        const DIVING: &[usize] = &[{diving}];
        if DIVING.contains(&(rule as usize)) {{ self.fuel }} else {{ 8 }}
    }}
    fn fire(&self, rule: u16, e: Redex, ctx: &mut Wctx) {{
        match rule {{
{fire_arms}            _ => unreachable!(\"rule {{}}\", rule),
        }}
    }}
    fn dive(&self, f: u16, args: &[u64], fuel: &mut i64, ctx: &mut Wctx) -> DiveResult {{
        // args[0] is the destination. A suspended dive has already spawned
        // its residue behind a record chain; attach the chain's root here.
        let parent = args[0];
        let r = match f as usize {{
{dive_arms}            _ => unreachable!(\"dive {{}}\", f),
        }};
        match r {{
            Ok(v) => DiveResult::Done(v),
            Err(rec) => {{
                if parent == NONE {{
                    return DiveResult::Suspended(rec as u32);
                }}
                ctx.set_parent(rec as u32, parent);
                DiveResult::Suspended(NO_REC)
            }}
        }}
    }}
}}
"
    ));
    out.push_str(MAIN);
    out
}

/// The per-function CALL-rule fire arm: unpack (freeing the arg chain),
/// (par-fold split), dive. Arguments arrive owned; on completion the fire
/// reclaims the ones the dive form only borrowed (read-only parameters).
fn call_fn(
    m: &CoreModule,
    fid: u32,
    pf: Option<&fold::ParFold>,
    jr: u16,
    bor: &[Vec<bool>],
) -> String {
    let ar = m.fns[fid as usize].arity;
    let mut s = format!("fn fc_{fid}(ctx: &mut Wctx, e: Redex) {{\nlet parent = e.aux;\n");
    match ar {
        0 => {}
        1 => s.push_str("let v0 = e.a;\n"),
        2 => s.push_str("let v0 = e.a;\nlet v1 = e.b;\n"),
        _ => {
            s.push_str("let v0 = e.a;\nlet mut ch = e.b;\n");
            for i in 1..ar {
                s.push_str(&format!(
                    "let v{i} = {{ let c = ctx.cell((ch - 1) as u32); ctx.free((ch - 1) as u32); ch = c[1]; c[0] }};\n"
                ));
            }
        }
    }
    if let Some(pf) = pf {
        s.push_str(&fold::split_snippet(fid, ar, pf, jr));
    }
    let args: String = (0..ar).map(|i| format!(", v{i}")).collect();
    let _ = bor;
    s.push_str(&format!(
        "match ctx.dive({fid}u16, &[parent{args}]) {{\nDiveResult::Done(v) => ctx.deliver(parent, v),\nDiveResult::Suspended(_) => {{}}\n}}\n}}\n\n"
    ));
    s
}

// ---- Core walkers shared by the emitters ----

thread_local! {
    /// Set for the module being emitted: `BOUNDED[f]` when every call path
    /// out of `f` is acyclic, so `f` can never run out of fuel and a call to
    /// it is an ordinary expression (no fuel check, no capture).
    static BOUNDED: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Functions on no call cycle (fixpoint over the call graph).
pub(crate) fn bounded_fns(m: &CoreModule) -> Vec<bool> {
    fn callees(e: &Core, out: &mut std::collections::HashSet<u32>) {
        match e {
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
            Core::Call(g, xs) => {
                out.insert(*g);
                xs.iter().for_each(|x| callees(x, out));
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                callees(a, out);
                callees(b, out);
            }
            Core::If(a, b, c) => {
                callees(a, out);
                callees(b, out);
                callees(c, out);
            }
            Core::Let(_, r, b) => {
                callees(r, out);
                callees(b, out);
            }
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().for_each(|x| callees(x, out)),
            Core::Match(s, arms) => {
                callees(s, out);
                arms.iter().for_each(|(_, _, b)| callees(b, out));
            }
            Core::Proj(a, _) => callees(a, out),
            Core::Lam(_, a) => callees(a, out),
            Core::App(f_, a_) => { callees(f_, out); callees(a_, out); }
        }
    }
    let cs: Vec<std::collections::HashSet<u32>> = m
        .fns
        .iter()
        .map(|f| {
            let mut s = std::collections::HashSet::new();
            callees(&f.body, &mut s);
            s
        })
        .collect();
    let n = m.fns.len();
    let mut b = vec![false; n];
    loop {
        let mut changed = false;
        for f in 0..n {
            if !b[f] && cs[f].iter().all(|g| (*g as usize) != f && b[*g as usize]) {
                b[f] = true;
                changed = true;
            }
        }
        if !changed {
            return b;
        }
    }
}

/// `from` (transitively) calls `to`.
fn reaches(m: &CoreModule, from: u32, to: u32) -> bool {
    let mut seen = vec![false; m.fns.len()];
    let mut stack = vec![from];
    while let Some(g) = stack.pop() {
        if std::mem::replace(&mut seen[g as usize], true) {
            continue;
        }
        let cs = callees(&m.fns[g as usize].body);
        if cs.contains(&to) {
            return true;
        }
        stack.extend(cs);
    }
    false
}

/// Whether `e` contains any call at all (`has_call` counts only calls
/// that may suspend).
pub(crate) fn any_call(e: &Core) -> bool {
    match e {
        Core::Call(..) => true,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => any_call(a) || any_call(b),
        Core::If(a, b, c) => any_call(a) || any_call(b) || any_call(c),
        Core::Tuple(xs) | Core::Ctor(_, xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().any(any_call),
        Core::Match(sc, arms) => any_call(sc) || arms.iter().any(|(_, _, b)| any_call(b)),
        Core::Proj(b, _) => any_call(b),
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
        Core::Lam(_, b) => any_call(b),
        Core::App(f_, a_) => any_call(f_) || any_call(a_),
    }
}

/// Inlining attribute for an emitted function: a small call-free body
/// (bounded work, typically a loop body helper) always inlines into its
/// callers; rustc's heuristic declines multi-site helpers.
pub(crate) fn inline_attr(body: &Core) -> &'static str {
    const MAX: usize = 192;
    let any = any_call(body);
    if !any && rewrite::size(body) <= MAX {
        "#[inline(always)]\n"
    } else {
        ""
    }
}

/// `inline_attr` plus: a loop helper desugared from a `while`/`for` with a
/// single call site from another function is that function's own loop, so
/// it always inlines there (without this the backend picks which member of
/// a recursive cycle absorbs the other by accident of ordering).
pub(crate) fn inline_attr_fn(m: &CoreModule, fid: u32) -> &'static str {
    let f = &m.fns[fid as usize];
    let a = inline_attr(&f.body);
    if !a.is_empty() {
        return a;
    }
    fn count(e: &Core, g: u32, n: &mut usize) {
        match e {
            Core::Call(h, xs) => {
                *n += (*h == g) as usize;
                xs.iter().for_each(|x| count(x, g, n));
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) | Core::Let(_, a, b) => {
                count(a, g, n);
                count(b, g, n);
            }
            Core::If(a, b, c) => {
                count(a, g, n);
                count(b, g, n);
                count(c, g, n);
            }
            Core::Tuple(xs) | Core::Ctor(_, xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().for_each(|x| count(x, g, n)),
            Core::Match(sc, arms) => {
                count(sc, g, n);
                arms.iter().for_each(|(_, _, b)| count(b, g, n));
            }
            Core::Proj(b, _) => count(b, g, n),
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
            Core::Lam(_, b) => count(b, g, n),
            Core::App(f_, a_) => { count(f_, g, n); count(a_, g, n); }
        }
    }
    if !(f.name.starts_with("__while") || f.name.starts_with("__for")) {
        return "";
    }
    let mut n = 0;
    let mut caller = None;
    for (h, other) in m.fns.iter().enumerate() {
        if h as u32 != fid {
            let before = n;
            count(&other.body, fid, &mut n);
            if n > before {
                caller = Some(h as u32);
            }
        }
    }
    // only on a recursive cycle with its caller (the helper reaches the
    // caller again): there the backend must pick which member absorbs the
    // other, and the source says the loop belongs to its function.
    // Elsewhere the backend's own inlining decision stands.
    let Some(c) = caller else { return "" };
    if !reaches(m, fid, c) {
        return "";
    }
    if std::env::var_os("MITHRIL_DEBUG_INLINE").is_some() {
        eprintln!("loop helper {fid} {}: {n} call sites", f.name);
    }
    if n == 1 {
        "#[inline(always)]\n"
    } else {
        ""
    }
}

pub(crate) fn is_bounded(g: u32) -> bool {
    BOUNDED.with(|b| b.borrow().get(g as usize).copied().unwrap_or(false))
}

/// Whether evaluating `e` may run a suspendable call (calls to bounded
/// functions are plain expressions).
/// Functions called anywhere in `e`.
fn callees(e: &Core) -> std::collections::HashSet<u32> {
    fn go(e: &Core, out: &mut std::collections::HashSet<u32>) {
        match e {
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
            Core::Call(g, xs) => {
                out.insert(*g);
                xs.iter().for_each(|x| go(x, out));
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                go(a, out);
                go(b, out);
            }
            Core::If(a, b, c) => {
                go(a, out);
                go(b, out);
                go(c, out);
            }
            Core::Let(_, r, b) => {
                go(r, out);
                go(b, out);
            }
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().for_each(|x| go(x, out)),
            Core::Match(s, arms) => {
                go(s, out);
                arms.iter().for_each(|(_, _, b)| go(b, out));
            }
            Core::Proj(a, _) => go(a, out),
            Core::Lam(_, a) => go(a, out),
            Core::App(f_, a_) => { go(f_, out); go(a_, out); }
        }
    }
    let mut out = std::collections::HashSet::new();
    go(e, &mut out);
    out
}

/// Two or more self calls, not all in tail position: recursion that forks.
fn fork_recursive(fid: u32, f: &mithril_front::core::CoreFn) -> bool {
    fn n(fid: u32, e: &Core) -> usize {
        match e {
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => 0,
            Core::Call(g, xs) => usize::from(*g == fid) + xs.iter().map(|x| n(fid, x)).sum::<usize>(),
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => n(fid, a) + n(fid, b),
            Core::If(a, b, c) => n(fid, a) + n(fid, b) + n(fid, c),
            Core::Let(_, r, b) => n(fid, r) + n(fid, b),
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().map(|x| n(fid, x)).sum(),
            Core::Match(s, arms) => n(fid, s) + arms.iter().map(|(_, _, b)| n(fid, b)).sum::<usize>(),
            Core::Proj(a, _) => n(fid, a),
            Core::Lam(_, a) => n(fid, a),
            Core::App(f_, a_) => n(fid, a_) + n(fid, f_),
        }
    }
    !f.self_tail_rec && n(fid, &f.body) >= 2
}

pub(crate) fn has_call(e: &Core) -> bool {
    match e {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
        Core::Call(g, xs) => !is_bounded(*g) || xs.iter().any(has_call),
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => has_call(a) || has_call(b),
        Core::If(a, b, c) => has_call(a) || has_call(b) || has_call(c),
        Core::Let(_, r, b) => has_call(r) || has_call(b),
        Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().any(has_call),
        Core::Match(s, arms) => has_call(s) || arms.iter().any(|(_, _, b)| has_call(b)),
        Core::Proj(a, _) => has_call(a),
        Core::Lam(_, a) => has_call(a),
        Core::App(f_, a_) => has_call(f_) || has_call(a_),
    }
}

pub(crate) fn max_var(e: &Core) -> u32 {
    fn go(e: &Core, m: &mut u32) {
        match e {
            Core::Var(i) => *m = (*m).max(*i),
            Core::Num(_) | Core::Flo(_) => {}
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                go(a, m);
                go(b, m);
            }
            Core::If(a, b, c) => {
                go(a, m);
                go(b, m);
                go(c, m);
            }
            Core::Let(x, r, b) => {
                *m = (*m).max(*x);
                go(r, m);
                go(b, m);
            }
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                xs.iter().for_each(|x| go(x, m))
            }
            Core::Match(s, arms) => {
                go(s, m);
                for (_, bs, b) in arms {
                    for bv in bs {
                        *m = (*m).max(*bv);
                    }
                    go(b, m);
                }
            }
            Core::Proj(a, _) => go(a, m),
            Core::Lam(_, a) => go(a, m),
            Core::App(f_, a_) => { go(f_, m); go(a_, m); }
        }
    }
    let mut m = 0;
    go(e, &mut m);
    m
}

pub(crate) fn free_vars(e: &Core) -> BTreeSet<u32> {
    fn go(e: &Core, bound: &mut Vec<u32>, out: &mut BTreeSet<u32>) {
        match e {
            Core::Num(_) | Core::Flo(_) => {}
            Core::Var(i) => {
                if !bound.contains(i) {
                    out.insert(*i);
                }
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                go(a, bound, out);
                go(b, bound, out);
            }
            Core::If(a, b, c) => {
                go(a, bound, out);
                go(b, bound, out);
                go(c, bound, out);
            }
            Core::Let(x, r, b) => {
                go(r, bound, out);
                bound.push(*x);
                go(b, bound, out);
                bound.pop();
            }
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                for x in xs {
                    go(x, bound, out);
                }
            }
            Core::Match(s, arms) => {
                go(s, bound, out);
                for (_, bs, b) in arms {
                    let n = bound.len();
                    bound.extend(bs.iter().copied());
                    go(b, bound, out);
                    bound.truncate(n);
                }
            }
            Core::Proj(a, _) => go(a, bound, out),
            Core::Lam(_, a) => go(a, bound, out),
            Core::App(f_, a_) => { go(f_, bound, out); go(a_, bound, out); }
        }
    }
    let mut out = BTreeSet::new();
    go(e, &mut Vec::new(), &mut out);
    out
}

// ---- use counting (linear cell discipline) ----

/// Remaining-use counts per variable.
pub(crate) type Cnt = std::collections::HashMap<u32, i64>;

/// Count every variable occurrence in an expression (sum over subtrees).
pub(crate) fn cnt_expr(e: &Core, m: &mut Cnt) {
    match e {
        Core::Var(i) => *m.entry(*i).or_insert(0) += 1,
        Core::Num(_) | Core::Flo(_) => {}
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            cnt_expr(a, m);
            cnt_expr(b, m);
        }
        Core::If(c, t, f) => {
            cnt_expr(c, m);
            cnt_expr(t, m);
            cnt_expr(f, m);
        }
        Core::Let(_, r, b) => {
            cnt_expr(r, m);
            cnt_expr(b, m);
        }
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
            xs.iter().for_each(|x| cnt_expr(x, m))
        }
        Core::Match(s, arms) => {
            cnt_expr(s, m);
            for (_, _, b) in arms {
                cnt_expr(b, m);
            }
        }
        Core::Proj(a, _) => cnt_expr(a, m),
        Core::Lam(_, a) => cnt_expr(a, m),
        Core::App(f_, a_) => { cnt_expr(f_, m); cnt_expr(a_, m); }
    }
}

/// Fold branch counts into `into` taking the per-variable maximum across
/// branches (only one branch executes).
pub(crate) fn merge_max(into: &mut Cnt, branches: Vec<Cnt>) {
    let mut mx = Cnt::new();
    for b in branches {
        for (k, v) in b {
            let e = mx.entry(k).or_insert(0);
            if v > *e {
                *e = v;
            }
        }
    }
    for (k, v) in mx {
        *into.entry(k).or_insert(0) += v;
    }
}

/// Use counts of a dive-form body: tail If/Match branches merge by max.
pub(crate) fn cnt_dive(e: &Core, m: &mut Cnt) {
    match e {
        Core::Let(_, r, b) => {
            cnt_expr(r, m);
            cnt_dive(b, m);
        }
        Core::If(c, t, f) => {
            cnt_expr(c, m);
            let mut mt = Cnt::new();
            cnt_dive(t, &mut mt);
            let mut mf = Cnt::new();
            cnt_dive(f, &mut mf);
            merge_max(m, vec![mt, mf]);
        }
        Core::Match(s, arms) => {
            cnt_expr(s, m);
            let bs: Vec<Cnt> = arms
                .iter()
                .map(|(_, _, b)| {
                    let mut mm = Cnt::new();
                    cnt_dive(b, &mut mm);
                    mm
                })
                .collect();
            merge_max(m, bs);
        }
        Core::Call(_, xs) => xs.iter().for_each(|x| cnt_expr(x, m)),
        other => cnt_expr(other, m),
    }
}

// ---- borrow inference (read-only parameters) ----
//
// A parameter is *borrowed* when the function (and everything derived from
// it via match binders and var aliases) never needs ownership: it is only
// matched on, projected, or lent onward to other borrowed parameters. A
// value derived from a borrowed parameter that escapes into a constructor,
// a tuple, or an owned call argument forces the parameter to be owned.
// Escaping reads elsewhere (returns, ops) are deep-copied at the use site,
// so they do not force ownership. Fixpoint over the module, initialized
// optimistically (everything borrowed).

/// ADT classes some value of which is used more than once (on one path)
/// somewhere in the program: those carry refcounts regardless.
fn shared_classes(m: &CoreModule, bodies: &[Core], tys: &ty::Types) -> std::collections::HashSet<u32> {
    let mut out = std::collections::HashSet::new();
    for (fid, b) in bodies.iter().enumerate() {
        let mut uses = Cnt::new();
        cnt_dive(b, &mut uses);
        for (v, n) in uses {
            if n >= 2 {
                if let ty::Ty::Adt(c) = tys.var(fid, v) {
                    out.insert(c);
                }
            }
        }
    }
    let _ = m;
    out
}

/// The function matches on parameter `p` and, in that match, builds a
/// constructor with the same number of fields as the matched one: owning
/// `p` lets the cell be rebuilt in place (Perceus/Koka: borrowing would
/// trade that reuse for a later teardown by the lender).
fn reuses_param(body: &Core, p: u32, m: &CoreModule, unbox: &std::collections::HashMap<u32, u8>) -> bool {
    fn builds(e: &Core, ar: usize, m: &CoreModule, unbox: &std::collections::HashMap<u32, u8>) -> bool {
        let builds = |e: &Core, ar: usize, m: &CoreModule| builds(e, ar, m, unbox);
        match e {
            Core::Ctor(c, xs) | Core::Reuse(_, c, xs) => {
                (*c as usize) < m.ctors.len() && m.ctors[*c as usize].1 == ar && ar > 0 && !unbox.contains_key(c)
                    || xs.iter().any(|x| builds(x, ar, m))
            }
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => builds(a, ar, m) || builds(b, ar, m),
            Core::If(a, b, c) => builds(a, ar, m) || builds(b, ar, m) || builds(c, ar, m),
            Core::Let(_, r, b) => builds(r, ar, m) || builds(b, ar, m),
            Core::Call(_, xs) | Core::Tuple(xs) | Core::Prim(_, xs) => xs.iter().any(|x| builds(x, ar, m)),
            Core::Match(s, arms) => builds(s, ar, m) || arms.iter().any(|(_, _, b)| builds(b, ar, m)),
            Core::Proj(a, _) => builds(a, ar, m),
            Core::Lam(_, a) => builds(a, ar, m),
            Core::App(f_, a_) => builds(f_, ar, m) || builds(a_, ar, m),
        }
    }
    fn go(e: &Core, p: u32, m: &CoreModule, unbox: &std::collections::HashMap<u32, u8>) -> bool {
        let go = |e: &Core, p: u32, m: &CoreModule| go(e, p, m, unbox);
        match e {
            Core::Match(s, arms) => {
                if **s == Core::Var(p) {
                    for (c, _, body) in arms {
                        let ar = m.ctors.get(*c as usize).map(|x| x.1).unwrap_or(0);
                        if ar > 0 && !unbox.contains_key(c) && builds(body, ar, m, unbox) {
                            return true;
                        }
                    }
                }
                go(s, p, m) || arms.iter().any(|(_, _, b)| go(b, p, m))
            }
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => go(a, p, m) || go(b, p, m),
            Core::If(a, b, c) => go(a, p, m) || go(b, p, m) || go(c, p, m),
            Core::Let(_, r, b) => go(r, p, m) || go(b, p, m),
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().any(|x| go(x, p, m)),
            Core::Proj(a, _) => go(a, p, m),
            Core::Lam(_, a) => go(a, p, m),
            Core::App(f_, a_) => go(f_, p, m) || go(a_, p, m),
        }
    }
    go(body, p, m, unbox)
}

fn borrows(m: &CoreModule, bodies: &[Core], tys: &ty::Types, unbox: &std::collections::HashMap<u32, u8>) -> (Vec<Vec<bool>>, Vec<std::collections::HashSet<u32>>) {
    // Only values of shared datatypes are lent. An int is an immediate, and
    // a linear (never shared) type moves for free and carries no refcount,
    // so a suspended borrower could not take a reference to it. For shared
    // types an escaping read of a borrowed value is an O(1) increment, so a
    // suspension that stores one just takes a reference.
    let shared = shared_classes(m, bodies, tys);
    let nf = m.fns.len();
    let mut bor: Vec<Vec<bool>> = m
        .fns
        .iter()
        .enumerate()
        .map(|(fid, f)| {
            (0..f.arity as u32)
                .map(|p| {
                    f.arity <= 60
                        && (matches!(tys.var(fid, p), ty::Ty::Adt(c) if shared.contains(&c))
                            || matches!(tys.var(fid, p), ty::Ty::Arr(_)))
                        && !reuses_param(&bodies[fid], p, m, unbox)
                })
                .collect()
        })
        .collect();
    for _ in 0..32 {
        let mut changed = false;
        for f in 0..nf {
            let esc = escape_mask(&bodies[f], m.fns[f].arity, &bor[f], &bor);
            for (i, b) in bor[f].iter_mut().enumerate() {
                if *b && esc & (1u64 << i) != 0 {
                    *b = false;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let bsets = (0..nf).map(|f| derive_set(&bodies[f], &bor[f])).collect();
    (bor, bsets)
}
fn escape_mask(body: &Core, arity: usize, own_bor: &[bool], bor: &[Vec<bool>]) -> u64 {
    use std::collections::HashMap;
    if arity > 60 {
        return u64::MAX;
    }
    let mut mask: HashMap<u32, u64> = HashMap::new();
    for (i, b) in own_bor.iter().enumerate() {
        if *b {
            mask.insert(i as u32, 1u64 << i);
        }
    }
    let mut esc = 0u64;
    fn var_mask(e: &Core, mask: &HashMap<u32, u64>) -> u64 {
        match e {
            Core::Var(i) => mask.get(i).copied().unwrap_or(0),
            _ => 0,
        }
    }
    fn walk(e: &Core, mask: &mut HashMap<u32, u64>, esc: &mut u64, bor: &[Vec<bool>]) {
        match e {
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                walk(a, mask, esc, bor);
                walk(b, mask, esc, bor);
            }
            Core::If(c, t, f) => {
                walk(c, mask, esc, bor);
                walk(t, mask, esc, bor);
                walk(f, mask, esc, bor);
            }
            Core::Let(x, r, b) => {
                if let Core::Var(y) = r.as_ref() {
                    let v = mask.get(y).copied().unwrap_or(0);
                    mask.insert(*x, v);
                }
                walk(r, mask, esc, bor);
                walk(b, mask, esc, bor);
            }
            // an array read only looks at its array: not an escape
            Core::Prim(mithril_front::core::Prim::ArrGet | mithril_front::core::Prim::ArrLen, xs) => {
                for (j, x) in xs.iter().enumerate() {
                    if j > 0 {
                        *esc |= var_mask(x, mask);
                    }
                    walk(x, mask, esc, bor);
                }
            }
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                for x in xs {
                    *esc |= var_mask(x, mask);
                    walk(x, mask, esc, bor);
                }
            }
            Core::Call(g, xs) => {
                for (j, x) in xs.iter().enumerate() {
                    let owned_param =
                        bor.get(*g as usize).map(|ps| !ps.get(j).copied().unwrap_or(false)).unwrap_or(true);
                    if owned_param {
                        *esc |= var_mask(x, mask);
                    }
                    walk(x, mask, esc, bor);
                }
            }
            Core::Match(s, arms) => {
                let sm = var_mask(s, mask);
                walk(s, mask, esc, bor);
                for (_, binders, b) in arms {
                    for bv in binders {
                        mask.insert(*bv, sm);
                    }
                    walk(b, mask, esc, bor);
                }
            }
            Core::Proj(a, _) => walk(a, mask, esc, bor),
            Core::Lam(_, a) => walk(a, mask, esc, bor),
            Core::App(f_, a_) => { walk(f_, mask, esc, bor); walk(a_, mask, esc, bor); }
        }
    }
    walk(body, &mut mask, &mut esc, bor);
    esc
}

/// Variables derived from borrowed parameters (raw reads, owner upstream).
fn derive_set(body: &Core, own_bor: &[bool]) -> std::collections::HashSet<u32> {
    let mut s: std::collections::HashSet<u32> =
        own_bor.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i as u32).collect();
    fn walk(e: &Core, s: &mut std::collections::HashSet<u32>) {
        match e {
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                walk(a, s);
                walk(b, s);
            }
            Core::If(c, t, f) => {
                walk(c, s);
                walk(t, s);
                walk(f, s);
            }
            Core::Let(x, r, b) => {
                if let Core::Var(y) = r.as_ref() {
                    if s.contains(y) {
                        s.insert(*x);
                    }
                }
                walk(r, s);
                walk(b, s);
            }
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Call(_, xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                xs.iter().for_each(|x| walk(x, s))
            }
            Core::Match(sc, arms) => {
                let inb = matches!(sc.as_ref(), Core::Var(y) if s.contains(y));
                walk(sc, s);
                for (_, binders, b) in arms {
                    if inb {
                        for bv in binders {
                            s.insert(*bv);
                        }
                    }
                    walk(b, s);
                }
            }
            Core::Proj(a, _) => walk(a, s),
            Core::Lam(_, a) => walk(a, s),
            Core::App(f_, a_) => { walk(f_, s); walk(a_, s); }
        }
    }
    walk(body, &mut s);
    s
}

// ---- generated-program templates ----

const PRELUDE: &str = r#"// GENERATED by mithril-codegen. Do not edit.
#![allow(unused, unused_mut, unreachable_code, unreachable_patterns, non_snake_case, clippy::all)]
use mithril_rt::prelude::*;
use mithril_rt::{DiveResult, Engine, Program, Redex, Wctx, NO_REC, ROOT};

/// Constructor allocation; arity <= 2 direct, wider ctors chain cells
/// (slot 0 = field, slot 1 = continuation con). The arity nibble saturates
/// at 15: any nibble > 2 means "one field + a link", so arbitrary widths
/// chain fine and walkers never rely on the nibble as an exact total.
fn mk_con(ctx: &mut Wctx, k: u16, fs: &[u64]) -> u64 {
    let n = fs.len();
    if n == 0 { return con(0, k, 0); }
    if n <= 2 {
        let a = calloc(ctx, k, fs[0], if n == 2 { fs[1] } else { 0 });
        return con(a, k, n as u8);
    }
    let a = calloc(ctx, k, fs[n - 2], fs[n - 1]);
    let mut chain = con(a, k, 2);
    let mut i = n - 2;
    while i > 0 {
        i -= 1;
        let a = calloc(ctx, k, fs[i], chain);
        chain = con(a, k, (n - i).min(15) as u8);
    }
    chain
}

/// Constructor k is statically linear (see LIN): never shared, no rc.
#[inline(always)] fn lin(k: u16) -> bool { if k == 0xFFF { LIN_TUP } else { LIN[k as usize] } }

#[inline(always)] fn calloc(ctx: &mut Wctx, k: u16, a: u64, b: u64) -> u32 { if lin(k) { ctx.alloc_lin(a, b) } else { ctx.alloc(a, b) } }

#[inline(always)] fn mk_con1(ctx: &mut Wctx, k: u16, f0: u64) -> u64 { let a = calloc(ctx, k, f0, 0); con(a, k, 1) }

#[inline(always)] fn mk_con2(ctx: &mut Wctx, k: u16, f0: u64, f1: u64) -> u64 { let a = calloc(ctx, k, f0, f1); con(a, k, 2) }

/// Consume an arity<=2 constructor: move both fields out. Unique owner
/// moves raw and frees the cell; shared increfs the fields and decrefs the
/// root. (Chained arity>2 ctors take the generic dup+free path instead.)
#[inline(always)]
fn consume2(ctx: &mut Wctx, p: u64) -> (u64, u64) {
    let a = con_addr(p);
    let c = ctx.cell(a);
    if lin(con_tag(p)) || ctx.rc_unique(a) {
        ctx.free(a);
        (c[0], c[1])
    } else {
        let f0 = dup_val(ctx, c[0]);
        let f1 = dup_val(ctx, c[1]);
        let _ = ctx.rc_dec(a);
        (f0, f1)
    }
}

/// consume2 with the constructor statically known (the match arm's ctor):
/// `lin(k)` constant-folds, so linear types move with zero rc traffic.
#[inline(always)]
fn consume2k(ctx: &mut Wctx, p: u64, k: u16) -> (u64, u64) {
    let a = con_addr(p);
    let c = ctx.cell(a);
    if lin(k) || ctx.rc_unique(a) {
        ctx.free(a);
        (c[0], c[1])
    } else {
        let f0 = dup_val(ctx, c[0]);
        let f1 = dup_val(ctx, c[1]);
        let _ = ctx.rc_dec(a);
        (f0, f1)
    }
}

#[inline(always)]
fn consume2r(ctx: &mut Wctx, p: u64, k: u16) -> (u64, u64, u32) {
    let a = con_addr(p);
    let c = ctx.cell(a);
    if lin(k) || ctx.rc_unique(a) {
        (c[0], c[1], a)
    } else {
        let f0 = dup_val(ctx, c[0]);
        let f1 = dup_val(ctx, c[1]);
        let _ = ctx.rc_dec(a);
        (f0, f1, NOTOK)
    }
}

/// Consume a chained (arity > 2) constructor: move every field out and
/// free only the chain cells when this was the last reference (always,
/// for linear types); otherwise share the fields and drop the root.
#[inline(always)]
fn consume_chain<const N: usize>(ctx: &mut Wctx, p: u64, k: u16) -> [u64; N] {
    let a = con_addr(p);
    let mut out = [0u64; N];
    if N == 0 {
        return out; // nullary: no cell
    }
    if lin(k) || ctx.rc_unique(a) {
        let mut cur = a;
        for slot in out.iter_mut().take(N.saturating_sub(2)) {
            let c = ctx.cell(cur);
            ctx.free(cur);
            *slot = c[0];
            cur = con_addr(c[1]);
        }
        let c = ctx.cell(cur);
        ctx.free(cur);
        if N >= 2 {
            out[N - 2] = c[0];
            out[N - 1] = c[1];
        } else {
            out[0] = c[0];
        }
    } else {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = dup_val(ctx, field(ctx, p, i));
        }
        free_val(ctx, p);
    }
    out
}

/// Build a 2-field ctor in a reuse token's cell (or allocate).
#[inline(always)]
fn mk_con2r(ctx: &mut Wctx, tok: u32, k: u16, f0: u64, f1: u64) -> u64 {
    if tok == NOTOK {
        return mk_con2(ctx, k, f0, f1);
    }
    ctx.set(tok, 0, f0);
    ctx.set(tok, 1, f1);
    if !lin(k) {
        ctx.rc_set1(tok);
    }
    con(tok, k, 2)
}

/// Share a value: O(1) refcount bump on the root cell (immediates copy
/// for free; a >2-arity chain is owned by its first cell).
#[inline(always)]
fn dup_val(ctx: &mut Wctx, p: u64) -> u64 {
    match tag(p) {
        t if t >= TU => p,
        T_FLO => {
            ctx.rc_inc((p & M56) as u32);
            p
        }
        T_CON => {
            if con_ar(p) > 0 {
                debug_assert!(!lin(con_tag(p)), "share of a statically linear value");
                ctx.rc_inc(con_addr(p));
            }
            p
        }
        T_ARR => {
            arr_rc(p).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            p
        }
        _ => p,
    }
}

fn arr_new(ctx: &mut Wctx, n: i64, v: u64) -> u64 {
    if n < 0 {
        panic!("negative array size {}", n);
    }
    if tag(v) == T_NUM {
        return arr_new_raw(n, sh(v));
    }
    let n = n as usize;
    if !is_heap(v) {
        return arr_alloc_fill(n, v);
    }
    let p = arr_alloc(n);
    let e = arr_elems(p);
    for k in 0..n {
        // SAFETY: k < n, inside the block
        unsafe { *e.add(k) = if k + 1 == n { v } else { dup_val(ctx, v) } }
    }
    if n == 0 {
        free_val(ctx, v);
    } else {
        arr_mark_boxed(p, v);
    }
    p
}

#[inline(always)]
fn arr_get(ctx: &mut Wctx, a: u64, i: i64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    if arr_raw(a) {
        return arr_elem(a, i as usize);
    }
    // SAFETY: bounds checked
    dup_val(ctx, unsafe { *arr_elems(a).add(i as usize) })
}

#[inline(always)]
fn arr_set(ctx: &mut Wctx, a: u64, i: i64, v: u64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    let a = if arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1 { a } else { arr_copy(ctx, a) };
    if arr_raw(a) {
        if tag(v) == T_NUM {
            // SAFETY: bounds checked; `a` is now uniquely ours
            unsafe { *arr_elems(a).add(i as usize) = sh(v) as u64 };
            return a;
        }
        arr_unraw(a);
    }
    // SAFETY: bounds checked; `a` is now uniquely ours
    let slot = unsafe { arr_elems(a).add(i as usize) };
    free_val(ctx, unsafe { *slot });
    unsafe { *slot = v };
    arr_mark_boxed(a, v);
    a
}

#[inline(always)]
fn arr_set_i(ctx: &mut Wctx, a: u64, i: i64, v: u64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    let a = if arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1 { a } else { arr_copy(ctx, a) };
    debug_assert!(arr_raw(a) || n == 0);
    // SAFETY: bounds checked; `a` is now uniquely ours
    unsafe { *arr_elems(a).add(i as usize) = sh(v) as u64 };
    a
}

/// Take `a` as the unique reference (copy when shared).
#[inline(always)]
fn arr_own(ctx: &mut Wctx, a: u64) -> u64 {
    if arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1 { a } else { arr_copy(ctx, a) }
}

#[cold]
#[inline(never)]
fn arr_copy(ctx: &mut Wctx, a: u64) -> u64 {
    let n = arr_len_of(a);
    let b = arr_alloc(n);
    if arr_boxed(a) {
        for k in 0..n {
            // SAFETY: k < n in both blocks
            unsafe { *arr_elems(b).add(k) = dup_val(ctx, *arr_elems(a).add(k)) }
        }
        // SAFETY: as arr_rc; `b` is ours
        unsafe { *arr_block(b).add(1) |= ARR_BOXED }
    } else {
        // SAFETY: n elements in both blocks, distinct allocations
        unsafe { std::ptr::copy_nonoverlapping(arr_elems(a), arr_elems(b), n) }
        if arr_raw(a) {
            // SAFETY: as arr_rc; `b` is ours
            unsafe { *arr_block(b).add(1) |= ARR_RAW }
        }
    }
    free_val(ctx, a);
    b
}

fn arr_drop(ctx: &mut Wctx, p: u64) {
    if arr_rc(p).fetch_sub(1, std::sync::atomic::Ordering::AcqRel) != 1 {
        return;
    }
    if arr_boxed(p) {
        let n = arr_len_of(p);
        for k in 0..n {
            // SAFETY: k < n; the block is still allocated
            free_val(ctx, unsafe { *arr_elems(p).add(k) });
        }
    }
    arr_free_block(p);
}

/// Drop a reference; the last one tears the value down (frees the chain
/// cells and drops every field).
/// Drop a reference (immediates return at once; the teardown is out of line
/// so hot paths stay small).
#[inline(always)]
fn free_val(ctx: &mut Wctx, p: u64) {
    let t = tag(p);
    if t != T_CON && t != T_FLO && t != T_ARR {
        return;
    }
    free_val_slow(ctx, p);
}

/// Last use of a boxed value that is only projected: move field `i` out,
/// freeing the cells and dropping the other fields (a shared value keeps
/// its fields: share field `i`, drop this reference).
fn take_field(ctx: &mut Wctx, p: u64, i: usize) -> u64 {
    let root = con_addr(p);
    if !lin(con_tag(p)) && !ctx.rc_unique(root) {
        let f = dup_val(ctx, field(ctx, p, i));
        free_val(ctx, p);
        return f;
    }
    let (mut q, mut idx, mut out) = (p, 0usize, 0u64);
    loop {
        let ar = con_ar(q) as usize;
        let ca = con_addr(q);
        let c = ctx.cell(ca);
        ctx.free(ca);
        if ar > 2 {
            if idx == i {
                out = c[0];
            } else {
                free_val(ctx, c[0]);
            }
            idx += 1;
            q = c[1];
        } else {
            for (s, f) in c.iter().enumerate().take(ar) {
                if idx + s == i {
                    out = *f;
                } else {
                    free_val(ctx, *f);
                }
            }
            return out;
        }
    }
}

/// Move every field out of a boxed k-tuple at its last use, freeing its
/// cells (a shared tuple keeps its fields: share them, drop this reference).
fn untup<const K: usize>(ctx: &mut Wctx, p: u64) -> [u64; K] {
    let mut out = [0u64; K];
    if !lin(con_tag(p)) && !ctx.rc_unique(con_addr(p)) {
        for (i, o) in out.iter_mut().enumerate() {
            *o = dup_val(ctx, field(ctx, p, i));
        }
        free_val(ctx, p);
        return out;
    }
    let (mut q, mut idx) = (p, 0usize);
    loop {
        let ar = con_ar(q) as usize;
        let ca = con_addr(q);
        let c = ctx.cell(ca);
        ctx.free(ca);
        if ar > 2 {
            out[idx] = c[0];
            idx += 1;
            q = c[1];
        } else {
            out[idx..idx + ar].copy_from_slice(&c[..ar]);
            return out;
        }
    }
}

#[inline(never)]
fn free_val_slow(ctx: &mut Wctx, p: u64) {
    match tag(p) {
        T_ARR => arr_drop(ctx, p),
        t if t >= TU => {}
        T_FLO => {
            let a = (p & M56) as u32;
            if ctx.rc_dec(a) {
                ctx.free(a);
            }
        }
        T_CON => {
            if con_ar(p) == 0 {
                return;
            }
            let root = con_addr(p);
            if !lin(con_tag(p)) && !ctx.rc_dec(root) {
                return;
            }
            // last reference: free the chain, dropping each field
            let mut q = p;
            loop {
                let ar = con_ar(q) as usize;
                let ca = con_addr(q);
                let c = ctx.cell(ca);
                ctx.free(ca);
                if ar > 2 {
                    free_val_slow(ctx, c[0]);
                    q = c[1];
                } else {
                    for s in 0..ar {
                        free_val(ctx, c[s]);
                    }
                    return;
                }
            }
        }
        _ => {}
    }
}

fn bin(ctx: &mut Wctx, op: u8, a: u64, b: u64, own: u8) -> u64 {
    if tag(a) == T_NUM && tag(b) == T_NUM {
        let (x, y) = (as_i(a), as_i(b));
        return num(wrap56(match op {
            0 => x.wrapping_add(y),
            1 => x.wrapping_sub(y),
            2 => x.wrapping_mul(y),
            3 => x.wrapping_div(y),
            4 => floor_div(x, y),
            5 => py_mod(x, y),
            6 => x.wrapping_shl(y as u32),
            7 => x.wrapping_shr(y as u32),
            8 => x & y,
            9 => x | y,
            _ => x ^ y,
        }));
    }
    let (x, y) = (flo_val(ctx, a), flo_val(ctx, b));
    if own & 1 != 0 { free_val(ctx, a); }
    if own & 2 != 0 { free_val(ctx, b); }
    let v = match op { 0 => x + y, 1 => x - y, 2 => x * y, 3 => x / y, _ => panic!("op not defined on floats") };
    flo(ctx, v)
}

fn cmp(ctx: &mut Wctx, op: u8, a: u64, b: u64, own: u8) -> u64 {
    let o = if tag(a) == T_NUM && tag(b) == T_NUM {
        as_i(a).partial_cmp(&as_i(b))
    } else {
        if own & 1 != 0 { free_val(ctx, a); }
        if own & 2 != 0 { free_val(ctx, b); }
        flo_val(ctx, a).partial_cmp(&flo_val(ctx, b))
    };
    let o = o.expect("incomparable values (NaN?)");
    use std::cmp::Ordering::*;
    let r = match op {
        0 => o == Less,
        1 => o != Greater,
        2 => o == Greater,
        3 => o != Less,
        4 => o == Equal,
        _ => o != Equal,
    };
    num(if r { 1 } else { 0 })
}

/// n-tuple of int zeros (the identity of the additive fold combiners).
fn zeros(ctx: &mut Wctx, n: usize) -> u64 {
    let fs: Vec<u64> = (0..n).map(|_| num(0)).collect();
    mk_con(ctx, 0xFFF, &fs)
}

/// Post-run readback + printing of the delivered root value.
fn show(eng: &Engine, p: u64) -> String {
    match tag(p) {
        t if t >= TU => format!("C{}({})", UNBOX_CID[(t - TU) as usize], as_i(p)),
        T_NUM => as_i(p).to_string(),
        T_FLO => format!("{:?}", f64::from_bits(eng.cell((p & M56) as u32)[0])),
        T_CON => {
            let k = con_tag(p);
            let mut q = p;
            let mut fs: Vec<String> = Vec::new();
            if con_ar(p) > 0 {
                loop {
                    let ar = con_ar(q) as usize;
                    let c = eng.cell(con_addr(q));
                    if ar > 2 {
                        fs.push(show(eng, c[0]));
                        q = c[1];
                    } else {
                        for s in 0..ar {
                            fs.push(show(eng, c[s]));
                        }
                        break;
                    }
                }
            }
            if k == 0xFFF { format!("({})", fs.join(", ")) } else { format!("C{}({})", k, fs.join(", ")) }
        }
        T_ARR => {
            let n = arr_len_of(p);
            let fs: Vec<String> = (0..n).map(|k| show(eng, arr_elem(p, k))).collect();
            format!("[{}]", fs.join(", "))
        }
        _ => panic!("unprintable result port {:#x}", p),
    }
}

"#;

const MAIN: &str = r#"
fn main() {
    // threads = argv[1] (default 1), fuel = argv[2] (default 4096);
    // `--threads N` / `--fuel N` flag forms are accepted too.
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut threads: usize = 1;
    let mut fuel: i64 = -1;
    let mut pos: Vec<&str> = Vec::new();
    let mut it = a.iter();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--threads" => threads = it.next().and_then(|s| s.parse().ok()).unwrap_or(threads),
            "--fuel" => fuel = it.next().and_then(|s| s.parse().ok()).unwrap_or(fuel),
            other => pos.push(other),
        }
    }
    if let Some(p) = pos.first() { if let Ok(v) = p.parse() { threads = v; } }
    if let Some(p) = pos.get(1) { if let Ok(v) = p.parse() { fuel = v; } }
    // Suspension exposes work to other workers. With one worker it only
    // bounds dive depth, so suspend rarely; the dive runs on a thread with a
    // large reserved stack so that bound stays safe.
    if fuel < 0 {
        // one worker: nothing to hand work to, so never suspend (the runner
        // thread's 1 GiB stack bounds the recursion instead)
        fuel = if threads <= 1 { 1 << 40 } else { 16384 };
    }
    let h = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let pg = Pg { fuel: fuel.clamp(1, u32::MAX as i64) as u32 };
            let mut eng = Engine::new(threads, fuel);
            let root = eng.run(&pg, Redex { a: 0, b: 0, aux: ROOT });
            println!("{}", show(&eng, root));
            if std::env::var_os("MITHRIL_STATS").is_some() {
                let st = eng.stats();
                eprintln!("peak_cells={} live_peak={} waves={} rewrites={} arrays_live={}", st.peak_cells, st.live_peak, st.parallel_waves, st.rewrites, ARR_LIVE.load(std::sync::atomic::Ordering::Relaxed));
            }
        })
        .expect("spawn main runner");
    if h.join().is_err() {
        std::process::exit(101);
    }
}
"#;
