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
pub mod lir;
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
        Core::Ctor(c, xs) => Val::C(*c, std::sync::Arc::new(xs.iter().map(core_value).collect::<Option<Vec<_>>>()?)),
        Core::Tuple(xs) => Val::T(std::sync::Arc::new(xs.iter().map(core_value).collect::<Option<Vec<_>>>()?)),
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
    fn bound(x: u32, b: &Core, env: &mut std::collections::HashMap<u32, u32>, next: &mut u32) -> (u32, Core) {
        let nx = *next;
        *next += 1;
        let saved = env.insert(x, nx);
        let b2 = go(b, env, next);
        match saved {
            Some(v) => env.insert(x, v),
            None => env.remove(&x),
        };
        (nx, b2)
    }
    fn go(e: &Core, env: &mut std::collections::HashMap<u32, u32>, next: &mut u32) -> Core {
        match e {
            Core::Var(i) => Core::Var(*env.get(i).unwrap_or(i)),
            Core::Num(_) | Core::Flo(_) => e.clone(),
            Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(go(a, env, next)), Box::new(go(b, env, next))),
            Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(go(a, env, next)), Box::new(go(b, env, next))),
            Core::If(c, t, f) => Core::If(Box::new(go(c, env, next)), Box::new(go(t, env, next)), Box::new(go(f, env, next))),
            Core::Lam(x, b) => {
                let (nx, b2) = bound(*x, b, env, next);
                Core::Lam(nx, Box::new(b2))
            }
            Core::App(f, a) => Core::App(Box::new(go(f, env, next)), Box::new(go(a, env, next))),
            Core::Let(x, r, b) => {
                let r2 = go(r, env, next);
                let (nx, b2) = bound(*x, b, env, next);
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

/// What fires when a rule fires.
#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    /// rule 0: the entry of `main` with `ROOT` as its destination
    Boot(u32),
    /// the CALL rule of function `fid`
    Call(u32),
    /// a proven fold's join
    Join(u32),
    /// a continuation segment
    Seg(u16),
    /// a TRMC hole fill
    Hole(u16),
    /// the net region's generic redexes / its fill records
    Net,
    Fill,
}

/// A lowered program: every function as `lir`, the rule and dive tables,
/// the tables its value helpers branch on, and its net region. A backend
/// prints it.
pub struct LirProgram {
    pub fns: Vec<lir::FnDef>,
    /// rule id -> what it fires (dense: every id below `rules.len()`)
    pub rules: Vec<Rule>,
    /// rules whose firing may run a dive (charged a whole budget)
    pub diving: Vec<u16>,
    /// dive entry `fid`: (arity, borrowed params) -- lent arguments are
    /// released after the dive
    pub dives: Vec<(usize, Vec<bool>)>,
    /// proven folds: their `FOLD_EST_<fid>` / `FOLD_MEAS_<fid>` statics
    pub folds: Vec<u32>,
    /// ctor id -> statically linear
    pub lin: Vec<bool>,
    pub lin_tup: bool,
    /// unbox slot -> original ctor id
    pub unbox_cid: Vec<u32>,
    /// the net region's rules (the same rule table, run by the runtime)
    pub net_rule: u16,
    pub fill_rule: u16,
    /// `Some(value)`: the whole program reduced to a literal at compile time
    pub constant: Option<Val>,
    /// The net region: the derived program (entries with `NExpr` bodies)
    /// plus the closures compiled code builds; `net_live[e]` = entry `e`
    /// can be reached by a Ref at runtime and ships with the program.
    pub net: mithril_net::NetProg,
    pub net_live: Vec<bool>,
    /// the forwarding segment (a suspended `apply` delivers through it)
    pub fwd: u16,
}

/// Lower a specialized module (see `emit_rust`); `rust_program` prints the
/// result for the CPU. The returned module is the one the functions were
/// lowered from (after codegen's own Core shapes).
pub fn lower(m: &CoreModule) -> (LirProgram, CoreModule) {
    // Constant folding, inlining, branch selection and static evaluation
    // happened in the net (specialize); what remains here are the shapes
    // codegen itself owns: mutual tail recursion into loops, and
    // if-conversion of loop back-edges.
    let m_s = rewrite::tail_inline(m);
    let m_u = rewrite::if_convert(&uniquify(&m_s));
    // records_to_tuples (rewrite.rs) is parked: without native multi-value
    // returns in the dive form it only trades ctor cells for tuple chains.
    let m = &m_u;
    let empty = |constant: Option<Val>| LirProgram { fns: Vec::new(), rules: Vec::new(), diving: Vec::new(), dives: Vec::new(), folds: Vec::new(), lin: Vec::new(), lin_tup: true, unbox_cid: Vec::new(), net_rule: 0, fill_rule: 0, constant, net: mithril_net::NetProg::new(m), net_live: Vec::new(), fwd: 0 };
    // Const path: the whole program reduced to its value at compile time.
    if let Some(v) = core_value(&m.fns[m.main as usize].body) {
        return (empty(Some(v)), m_u.clone());
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
        scalar::SHIFTED.with(|l| *l.borrow_mut() = reps);
        let cx = scalar::needs_ctx(m, &sn);
        scalar::CTX.with(|c| *c.borrow_mut() = cx);
    }
    scalar::LEAF.with(|l| {
        *l.borrow_mut() = (0..m.fns.len()).map(|f| native[f] && !any_call(&m.fns[f].body)).collect()
    });
    BOUNDED.with(|b| *b.borrow_mut() = bounded_fns(m).iter().zip(&native).map(|(x, y)| *x || *y).collect());
    CLOSURES.with(|c| *c.borrow_mut() = Some(mithril_net::NetProg::new(m)));
    scalar::SIGS.with(|s| {
        *s.borrow_mut() = scal.iter().zip(&native).map(|(sig, n)| if *n { sig.clone() } else { None }).collect()
    });
    let (bor, bsets) = borrows(m, &bodies, &tys, &unbox);
    let iret: Vec<bool> = tys.ret.iter().map(|t| *t == ty::Ty::Int).collect();
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
    let fast_code: Vec<Option<lir::FnDef>> = (0..nf)
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
    let mut fns_code = String::new();
    // every IR function, in emission order (printed as it is produced so
    // a function's forms stay adjacent in the text)
    let mut fns: Vec<lir::FnDef> = Vec::new();
    let mut emit = |defs: Vec<lir::FnDef>, code: &mut String| {
        for d in defs {
            lir::rust::func(&d, code);
            fns.push(d);
        }
    };
    for fid in 0..nf {
        if scal[fid].is_some() {
            // native scalar form + bridging dive form (see scalar.rs)
            emit(scalar::scalar_fn(m, fid as u32, &scal, &bor, true), &mut fns_code);
        } else {
            emit(seq::dive_fn(m, fid as u32, &bodies[fid], &bor, &bsets[fid], &mut sq, fwd, &unbox, &tys, &iret, &shared), &mut fns_code);
        }
        if let Some(q) = &fast_code[fid] {
            emit(vec![q.clone()], &mut fns_code);
        }
        if let Some((p, c)) = dps[fid] {
            emit(vec![seq::dps_fn(m, fid as u32, p, c, &bodies[fid], &bor, &mut sq, &unbox, &tys, &iret, &shared)], &mut fns_code);
        }
        emit(vec![rules::expand_fn(m, fid as u32, &bodies[fid], &bor, &mut sq, &unbox, &tys, &iret, &shared), call_fn(m, fid as u32, folds[fid].as_ref(), join_rule[fid])], &mut fns_code);
        if let Some(pf) = &folds[fid] {
            emit(vec![fold::join_fn(fid as u32, pf)], &mut fns_code);
        }
    }
    // Segments may enqueue further segments while being emitted.
    let mut done = 0;
    while done < sq.q.len() {
        let seg = sq.q[done].clone();
        done += 1;
        emit(vec![rules::segment_fn(m, &seg, &bor, &mut sq, &unbox, &tys, &iret, &shared)], &mut fns_code);
    }

    // the net region: generic redexes and the records that feed a call's
    // result back into a wire
    let net_rule = sq.next;
    let fill_rule = sq.next + 1;
    let mut rules = vec![Rule::Net; sq.next as usize + 2];
    rules[0] = Rule::Boot(m.main);
    for fid in 0..nf {
        rules[1 + fid] = Rule::Call(fid as u32);
        if folds[fid].is_some() {
            rules[join_rule[fid] as usize] = Rule::Join(fid as u32);
        }
    }
    // rules whose firing may run a dive: CALL entries and segments with calls
    let mut diving: Vec<u16> = (1..=nf as u16).collect();
    for seg in &sq.q {
        rules[seg.id as usize] = Rule::Seg(seg.id);
        if has_call(&seg.body) {
            diving.push(seg.id);
        }
    }
    for (id, cid) in &sq.holes {
        rules[*id as usize] = Rule::Hole(*id);
        emit(vec![hole_fn(*id, *cid)], &mut fns_code);
    }
    rules[net_rule as usize] = Rule::Net;
    rules[fill_rule as usize] = Rule::Fill;
    diving.push(net_rule);
    diving.push(fill_rule);
    let mut unbox_cid = vec![0u32; unbox.len()];
    for (cid, slot) in &unbox {
        unbox_cid[*slot as usize] = *cid;
    }
    // static linearity: close the shared set over field types; every ctor
    // of an unshared class is linear (refcount never read or written)
    let (lin, lin_tup) = {
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
        ((0..m.ctors.len()).map(|c| !sh.poison && !sh.classes.contains(&tys.class_of[c])).collect(), !sh.poison && !sh.tuples)
    };
    let _ = fns_code;
    let net = CLOSURES.with(|c| c.borrow_mut().take()).expect("closure registry");
    let net_live = net_live_entries(&net, nf);
    let prog = LirProgram {
        fns,
        rules,
        diving,
        dives: (0..nf).map(|fid| (m.fns[fid].arity, bor[fid].clone())).collect(),
        folds: (0..nf as u32).filter(|f| folds[*f as usize].is_some()).collect(),
        lin,
        lin_tup,
        unbox_cid,
        net_rule,
        fill_rule,
        constant: None,
        net,
        net_live,
        fwd,
    };
    (prog, m_u.clone())
}

/// Only what a Ref in the net region can reach is instantiated at
/// runtime: the closures compiled code builds and, transitively, the
/// lifted branches/arms their bodies mention (a real function is run by
/// its CALL rule, never instantiated).
fn net_live_entries(np: &mithril_net::NetProg, nfns: usize) -> Vec<bool> {
    let mut live = vec![false; np.entries.len()];
    let mut work: Vec<usize> = (nfns..np.entries.len()).filter(|e| np.closure_entry(*e)).collect();
    fn refs(e: &mithril_net::NExpr, out: &mut Vec<usize>) {
        use mithril_net::NExpr::*;
        match e {
            Num(_) | Flo(_) | Var(_) => {}
            Op2(_, a, b) | Let(_, a, b) | App(a, b) => {
                refs(a, out);
                refs(b, out);
            }
            Call(f, xs) => {
                // a call whose arguments are not all produced when it is
                // met runs as a net: the callee's body must be shipped
                out.push(*f as usize);
                xs.iter().for_each(|x| refs(x, out));
            }
            Ctor(_, xs) | Tuple(xs) | Prim(_, xs) => xs.iter().for_each(|x| refs(x, out)),
            If(c, t, e2) => {
                refs(c, out);
                out.push(t.entry as usize);
                out.push(e2.entry as usize);
            }
            Match(s, _, specs) => {
                refs(s, out);
                specs.iter().for_each(|sp| out.push(sp.entry as usize));
            }
            Proj(a, _) | Lam(_, a) => refs(a, out),
        }
    }
    while let Some(e) = work.pop() {
        if live[e] {
            continue;
        }
        live[e] = true;
        refs(&np.entries[e].body, &mut work);
    }
    live
}

fn fns_code_of(prog: &LirProgram) -> String {
    let mut s = String::new();
    for d in &prog.fns {
        lir::rust::func(d, &mut s);
    }
    s
}

/// The CPU program: `lower` printed by `lir::rust`, plus the Rust glue
/// (prelude wrappers, tables, net region, the `Program` impl, `main`).
fn emit_rust_inner(m: &CoreModule) -> String {
    let (prog, _m) = lower(m);
    if let Some(v) = &prog.constant {
        return format!(
            "// GENERATED by mithril-codegen (net quiesced at compile time).\nfn main() {{\n    println!(\"{{}}\", {:?});\n}}\n",
            fmt_val(v)
        );
    }
    let (net_text, fns_code) = (net_region(&prog), fns_code_of(&prog));
    let mut fire_arms = String::new();
    for (id, r) in prog.rules.iter().enumerate() {
        let arm = match r {
            Rule::Boot(f) => format!("fc_{f}(ctx, e.a, e.b, ROOT)"),
            Rule::Call(f) => format!("fc_{f}(ctx, e.a, e.b, e.aux)"),
            Rule::Join(f) => format!("jn_{f}(ctx, e.a, e.b, e.aux)"),
            Rule::Seg(k) => format!("sg_{k}(ctx, e.a, e.b, e.aux)"),
            Rule::Hole(k) => format!("hl_{k}(ctx, e.a, e.b, e.aux)"),
            Rule::Net => "net_fire(ctx, e)".to_string(),
            Rule::Fill => "fill_fire(ctx, e)".to_string(),
        };
        fire_arms.push_str(&format!("            {id} => {arm},\n"));
    }
    let diving: String = prog.diving.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(", ");
    let mut dive_arms = String::new();
    for (fid, (ar, bor)) in prog.dives.iter().enumerate() {
        let args: String = (0..*ar).map(|i| format!(", args[{}]", i + 1)).collect();
        // the owned-argument entry: values lent to borrowed parameters are
        // released once the dive returns, finished or suspended (a
        // suspension took its own references to what it still reads)
        let lent: String = (0..*ar).filter(|&i| bor[i]).map(|i| format!("free_val(ctx, args[{}]);\n", i + 1)).collect();
        if lent.is_empty() {
            dive_arms.push_str(&format!("            {fid} => d_{fid}(ctx, fuel{args}),\n"));
        } else {
            dive_arms.push_str(&format!("            {fid} => {{\nlet r = d_{fid}(ctx, fuel{args});\n{lent}r\n}}\n"));
        }
    }
    let (net_rule, n_rules) = (prog.net_rule, prog.rules.len());
    let mut out = String::with_capacity(fns_code.len() + 8192);
    out.push_str(PRELUDE);
    out.push_str(&format!(
        "const UNBOX_CID: [u32; {}] = [{}];\n",
        prog.unbox_cid.len(),
        prog.unbox_cid.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")
    ));
    out.push_str(&format!(
        "const LIN: [bool; {}] = [{}];\nconst LIN_TUP: bool = {};\n",
        prog.lin.len(),
        prog.lin.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", "),
        prog.lin_tup
    ));
    for fid in &prog.folds {
        out.push_str(&fold::est_static(*fid));
    }
    out.push_str(&fns_code);
    out.push_str(&net_text);
    out.push_str(&format!(
        "struct Pg {{ fuel: u32, entries: Vec<Entry>, metas: Vec<MatchMeta> }}

impl Program for Pg {{
    fn n_rules(&self) -> usize {{ {n_rules} }}
    fn net_rule(&self) -> u16 {{ {net_rule} }}
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
// ---- the net region of a compiled program ----

fn nexpr_src(e: &mithril_net::NExpr) -> String {
    use mithril_net::NExpr::*;
    let spec = |s: &mithril_net::ClosureSpec| format!("ClosureSpec {{ entry: {}, caps: vec![{}] }}", s.entry, s.caps.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "));
    let list = |xs: &[mithril_net::NExpr]| xs.iter().map(nexpr_src).collect::<Vec<_>>().join(", ");
    match e {
        Num(n) => format!("NExpr::Num({n}i64)"),
        Flo(f) => format!("NExpr::Flo(f64::from_bits({}u64))", f.to_bits()),
        Var(v) => format!("NExpr::Var({v})"),
        Op2(c, a, b) => format!("NExpr::Op2({c}, Box::new({}), Box::new({}))", nexpr_src(a), nexpr_src(b)),
        Let(v, r, b) => format!("NExpr::Let({v}, Box::new({}), Box::new({}))", nexpr_src(r), nexpr_src(b)),
        Call(f, xs) => format!("NExpr::Call({f}, vec![{}])", list(xs)),
        Ctor(t, xs) => format!("NExpr::Ctor({t}, vec![{}])", list(xs)),
        Tuple(xs) => format!("NExpr::Tuple(vec![{}])", list(xs)),
        If(c, t, e2) => format!("NExpr::If(Box::new({}), {}, {})", nexpr_src(c), spec(t), spec(e2)),
        Match(sc, mid, specs) => format!("NExpr::Match(Box::new({}), {mid}, vec![{}])", nexpr_src(sc), specs.iter().map(spec).collect::<Vec<_>>().join(", ")),
        Proj(a, mid) => format!("NExpr::Proj(Box::new({}), {mid})", nexpr_src(a)),
        Prim(c, xs) => format!("NExpr::Prim({c}, vec![{}])", list(xs)),
        Lam(x, b) => format!("NExpr::Lam({x}, Box::new({}))", nexpr_src(b)),
        App(f, a) => format!("NExpr::App(Box::new({}), Box::new({}))", nexpr_src(f), nexpr_src(a)),
    }
}

/// The program's net region: its entry table (the derived program, plus
/// the closures compiled code builds), the `Prog` half of the shared rule
/// table (a call unfolds into a spawned CALL rule whose result a FILL
/// record links back into the net; builtins compute on runtime values;
/// arrays and unboxed constructors are the runtime's value forms), the two
/// engine rules of the region, and the bridge compiled code uses: build a
/// closure, apply one (synchronously when it finishes within budget,
/// otherwise through a forwarding record).
fn net_region(prog: &LirProgram) -> String {
    let (np, live) = (&prog.net, &prog.net_live);
    let (nfns, net_rule, fill_rule, fwd) = (prog.dives.len(), prog.net_rule, prog.fill_rule, prog.fwd);
    let _ = net_rule;
    let mut s = String::new();
    s.push_str("fn net_entries() -> Vec<Entry> {\n    vec![\n");
    for (i, e) in np.entries.iter().enumerate() {
        if live[i] {
            s.push_str(&format!("        Entry {{ params: vec![{}], body: {} }},\n", e.params.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "), nexpr_src(&e.body)));
        } else {
            s.push_str("        Entry { params: vec![], body: NExpr::Num(0) },\n");
        }
    }
    s.push_str("    ]\n}\n\n");
    s.push_str(&format!("const NET_LIVE: [bool; {}] = [{}];\n\n", live.len(), live.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", ")));
    s.push_str("fn net_metas() -> Vec<MatchMeta> {\n    vec![\n");
    for mm in &np.metas {
        match mm {
            mithril_net::MatchMeta::Proj(i) => s.push_str(&format!("        MatchMeta::Proj({i}),\n")),
            mithril_net::MatchMeta::Arms(tags) => s.push_str(&format!("        MatchMeta::Arms(vec![{}]),\n", tags.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", "))),
        }
    }
    s.push_str("    ]\n}\n\n");
    s.push_str(&format!(r#"const NFNS: usize = {nfns};
static PG: std::sync::OnceLock<Pg> = std::sync::OnceLock::new();
fn pg_ref() -> &'static Pg {{ PG.get().expect("program not installed") }}

impl<'e> Prog<Wctx<'e>> for Pg {{
    fn unfold(&self, ctx: &mut Wctx<'e>, r: Port, other: Port) -> u64 {{
        let entry = ref_entry(r) as usize;
        let args = net_list_collect(ctx, ref_head(r));
        if entry < NFNS && args.iter().all(|a| a.tag() != Tag::Var) {{
            // a compiled function with every argument produced: its CALL
            // rule runs it (strict, native); the result comes back through
            // a FILL record that links it into `other`
            let ri = ctx.alloc_rec({fill_rule}u16, 1, other.0 as u32, (other.0 >> 32) as u32, NONE);
            let raw: Vec<u64> = args.iter().map(|p| p.0).collect();
            spawn_call(ctx, 1 + entry as u16, &raw, (ri as u64) << 3);
        }} else {{
            // a lifted branch/arm, or a call met before its arguments are
            // produced (inside a closure body being built): its body runs
            // as a net and waits on the wires like any agent
            assert!(entry < NET_LIVE.len() && NET_LIVE[entry], "ICE: entry {{}} is not shipped with the net region (ref {{:#x}}, other {{:?}})", entry, r.0, other.tag());
            ctx.instantiate(&self.entries, entry, args, other);
        }}
        1
    }}
    fn is_closure(&self, r: Port) -> bool {{
        ref_entry(r) as usize >= NFNS
    }}
    fn mat_meta(&self, mid: u16) -> MatMeta<'_> {{
        match &self.metas[mid as usize] {{
            MatchMeta::Proj(i) => MatMeta::Proj(*i),
            MatchMeta::Arms(tags) => MatMeta::Arms(tags),
        }}
    }}
    fn compute(&self, ctx: &mut Wctx<'e>, code: u16, x: Port, y: Port) -> Option<Port> {{
        net_compute(ctx, code, x.0, y.0).map(Port)
    }}
    fn park_op(&self, _ctx: &mut Wctx<'e>, op: Port, _y: Port) {{
        panic!("runtime: builtin {{}} could not compute", op.payload() & 0xFF)
    }}
    fn is_ext_value(&self, p: Port) -> bool {{
        let t = tag(p.0);
        t >= TU || t == T_ARR
    }}
    fn copy_ext(&self, ctx: &mut Wctx<'e>, p: Port) -> Port {{
        Port(dup_val(ctx, p.0))
    }}
    fn erase_ext(&self, ctx: &mut Wctx<'e>, p: Port) {{
        free_val(ctx, p.0)
    }}
    fn ext_ctor(&self, _ctx: &mut Wctx<'e>, p: Port) -> (u16, Vec<Port>) {{
        let t = tag(p.0);
        assert!(t >= TU, "runtime: match on an array");
        (UNBOX_CID[(t - TU) as usize] as u16, vec![Port(num(as_i(p.0)))])
    }}
    fn deliver(&self, ctx: &mut Wctx<'e>, kont: Port, val: Port) {{
        ctx.deliver(kont.payload(), val.0)
    }}
}}

/// Builtins on runtime values (the compile-time `compute` folds the same
/// codes on literals). Codes < 16 are int/float arithmetic, 16..22
/// comparisons, 32.. the `Prim`s, 44 the (index, value) pair of a set.
fn net_compute(ctx: &mut Wctx, code: u16, x: u64, y: u64) -> Option<u64> {{
    if code >= 32 {{
        return Some(match code {{
            32 => num(f32_add(as_i(x), as_i(y))),
            33 => num(f32_sub(as_i(x), as_i(y))),
            34 => num(f32_mul(as_i(x), as_i(y))),
            35 => num(f32_div(as_i(x), as_i(y))),
            36 => num(f32_sqrt(as_i(x))),
            37 => num(f32_lt(as_i(x), as_i(y))),
            38 => num(f32_from_u32(as_i(x))),
            39 => num(f32_to_u32(as_i(x))),
            40 => arr_new(ctx, as_i(x), y),
            41 => arr_get(ctx, x, as_i(y)),
            42 => num(arr_len_of(x) as i64),
            43 => {{
                let i = field(ctx, y, 0);
                let v = field(ctx, y, 1);
                ctx.free(con_addr(y));
                arr_set(ctx, x, as_i(i), v)
            }}
            44 => mk_con(ctx, 0xFFF, &[x, y]),
            c => unreachable!("builtin code {{}}", c),
        }});
    }}
    if tag(x) == T_NUM && tag(y) == T_NUM {{
        let (a, b) = (as_i(x), as_i(y));
        if code >= 16 {{
            let r = match code {{ 16 => a < b, 17 => a <= b, 18 => a > b, 19 => a >= b, 20 => a == b, 21 => a != b, _ => unreachable!() }};
            return Some(num(r as i64));
        }}
        if matches!(code, 3 | 4 | 5) && b == 0 {{
            panic!("runtime: division by zero");
        }}
        let r = match code {{
            0 => a.wrapping_add(b), 1 => a.wrapping_sub(b), 2 => a.wrapping_mul(b), 3 => a.wrapping_div(b),
            4 => floor_div(a, b), 5 => py_mod(a, b), 6 => a.wrapping_shl(b as u32), 7 => a.wrapping_shr(b as u32),
            8 => a & b, 9 => a | b, 10 => a ^ b, c => unreachable!("int opcode {{}}", c),
        }};
        return Some(num(wrap56(r)));
    }}
    let (a, b) = (flo_val(ctx, x), flo_val(ctx, y));
    ctx.free((x & M56) as u32);
    ctx.free((y & M56) as u32);
    if code >= 16 {{
        let r = match code {{ 16 => a < b, 17 => a <= b, 18 => a > b, 19 => a >= b, 20 => a == b, 21 => a != b, _ => unreachable!() }};
        return Some(num(r as i64));
    }}
    let r = match code {{ 0 => a + b, 1 => a - b, 2 => a * b, 3 => a / b, c => unreachable!("float opcode {{}}", c) }};
    Some(flo(ctx, r))
}}

/// Rule {net_rule}: a generic redex spilled to the engine (fuel-out, or a
/// value delivered into the net by a FILL record on another worker).
fn net_fire(ctx: &mut Wctx, e: Redex) {{
    ctx.net_push(Port(e.a), Port(e.b));
    let budget = ctx.fuel();
    ctx.reduce_net(pg_ref(), budget);
}}

/// Rule {fill_rule}: a compiled call's result links into the wire (or
/// agent) the net was waiting on.
fn fill_fire(ctx: &mut Wctx, e: Redex) {{
    let inf = ctx.rec(e.aux as u32);
    let target = Port(inf.d as u64 | ((inf.s as u64) << 32));
    net_link(ctx, Port(e.a), target);
    let budget = ctx.fuel();
    ctx.reduce_net(pg_ref(), budget);
}}

/// Sharing a closure value from compiled code is the net's DUP–LAM rule
/// (`copy_lam`): the original cell stays the first copy, so the caller's
/// port is still that closure; the second copy is returned. The bodies
/// are copied lazily by the rules as each copy is used.
fn dup_closure(ctx: &mut Wctx, p: u64) -> u64 {{
    let label = mithril_rt::mithril_core::agents::Cells::fresh_label(ctx);
    let (_, copy) = mithril_rt::mithril_core::rules::copy_lam(ctx, (p & M56) as u32, label);
    let budget = ctx.fuel();
    ctx.reduce_net(pg_ref(), budget);
    copy.0
}}

/// A closure value: entry `id` instantiated over its captured values.
fn build_closure(ctx: &mut Wctx, id: u16, caps: &[u64]) -> u64 {{
    let w = net_wire(ctx);
    ctx.instantiate(&pg_ref().entries, id as usize, caps.iter().map(|c| Port(*c)).collect(), w);
    let budget = ctx.fuel();
    ctx.reduce_net(pg_ref(), budget);
    let v = net_resolve(ctx, w);
    debug_assert!(v.tag() == Tag::Lam, "closure entry produced a {{:?}}", v.tag());
    v.0
}}

/// Apply a closure value from compiled code: the application is reduced
/// in the net region; a result within budget is returned, otherwise the
/// dive suspends on a forwarding record the result will be delivered to.
fn apply(ctx: &mut Wctx, f: u64, a: u64) -> R {{
    assert!(tag(f) == T_LAM, "apply: not a closure (tag {{}})", tag(f));
    let w = net_wire(ctx);
    let c = ctx.alloc(a, w.0);
    net_link(ctx, Port::new(Tag::App, c as u64), Port(f));
    let budget = ctx.fuel();
    ctx.reduce_net(pg_ref(), budget);
    let v = net_resolve(ctx, w);
    if v.tag() != Tag::Var {{
        return Ok(v.0);
    }}
    let r = ctx.alloc_rec({fwd}u16, 1, 0, 0, NONE);
    net_link(ctx, Port::new(Tag::Kont, (r as u64) << 3), v);
    Err(r as u64)
}}

/// Apply a closure value from the rule form: the result is delivered to
/// `parent` (a record slot) when the net produces it.
fn apply_spawn(ctx: &mut Wctx, f: u64, a: u64, parent: u64) {{
    assert!(tag(f) == T_LAM, "apply_spawn: not a closure (tag {{}})", tag(f));
    let c = ctx.alloc(a, Port::new(Tag::Kont, parent).0);
    net_link(ctx, Port::new(Tag::App, c as u64), Port(f));
    let budget = ctx.fuel();
    ctx.reduce_net(pg_ref(), budget);
}}

"#));
    s
}

fn call_fn(m: &CoreModule, fid: u32, pf: Option<&fold::ParFold>, jr: u16) -> lir::FnDef {
    use lir::{c, do_, let_, u16_, v, Ty, E};
    let ar = m.fns[fid as usize].arity;
    let mut s = vec![let_("parent", Ty::U64, v("aux"))];
    match ar {
        0 => {}
        1 => s.push(let_("v0", Ty::U64, v("a"))),
        2 => {
            s.push(let_("v0", Ty::U64, v("a")));
            s.push(let_("v1", Ty::U64, v("b")));
        }
        _ => {
            s.push(let_("v0", Ty::U64, v("a")));
            s.push(let_("ch", Ty::U64, v("b")));
            for i in 1..ar {
                s.push(let_(seq::vn(i as u32), Ty::U64, c("pop_chain", vec![E::Ref("ch".into())])));
            }
        }
    }
    if let Some(pf) = pf {
        s.extend(fold::split_snippet(fid, ar, pf, jr));
    }
    let mut args = vec![v("parent")];
    args.extend((0..ar).map(|i| v(seq::vn(i as u32))));
    s.push(do_(c("dive_to", vec![u16_(fid as u64), E::Slice(args)])));
    lir::FnDef { name: format!("fc_{fid}"), ctx: true, params: rules::rule_params(), ret: Ty::Unit, body: s, inline: lir::Inline::Default, cold: false }
}

/// The hole-fill rule of a TRMC ctor: the record holds the head cell
/// (`d`) and the pending hole cell (`s`); its one input fills the hole.
fn hole_fn(id: u16, cid: u32) -> lir::FnDef {
    use lir::{c, cast, do_, let_, p, u16_, u8_, usize_, v, Ty};
    let aux = || cast(v("aux"), Ty::U32);
    let body = vec![
        let_("s", Ty::U32, c("rec_s", vec![aux()])),
        do_(c("cell_set", vec![v("s"), usize_(1), v("a")])),
        do_(c("deliver", vec![c("rec_parent", vec![aux()]), p("con", vec![c("rec_d", vec![aux()]), u16_(cid as u64), u8_(2)])])),
    ];
    lir::FnDef { name: format!("hl_{id}"), ctx: true, params: rules::rule_params(), ret: Ty::Unit, body, inline: lir::Inline::Default, cold: false }
}

// ---- Core walkers shared by the emitters ----

thread_local! {
    /// The derived net program of the module being emitted: the closures
    /// compiled code builds are registered here as entries, and the whole
    /// table is shipped with the program for its net region.
    static CLOSURES: std::cell::RefCell<Option<mithril_net::NetProg>> = const { std::cell::RefCell::new(None) };
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
    e.any(&mut |e| match e {
        Core::Call(..) | Core::App(..) => Some(true),
        Core::Lam(..) => Some(false),
        _ => None,
    })
}

/// Inlining attribute for an emitted function: a small call-free body
/// (bounded work, typically a loop body helper) always inlines into its
/// callers; rustc's heuristic declines multi-site helpers.
pub(crate) fn inline_attr(body: &Core) -> &'static str {
    const MAX: usize = 192;
    let any = any_call(body);
    if !any && body.size() <= MAX {
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
        *n += e.sum(&mut |e| matches!(e, Core::Call(h, _) if *h == g) as usize);
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
    if n == 1 {
        "#[inline(always)]\n"
    } else {
        ""
    }
}

/// Register a closure built by compiled code: an entry over its free
/// variables (`caps`, sorted) whose body is the lambda itself.
pub(crate) fn closure_entry(caps: Vec<u32>, lam: &Core) -> u16 {
    CLOSURES.with(|c| c.borrow_mut().as_mut().expect("closure registry").add_entry(caps, lam))
}

pub(crate) fn is_bounded(g: u32) -> bool {
    BOUNDED.with(|b| b.borrow().get(g as usize).copied().unwrap_or(false))
}

/// Whether evaluating `e` may run a suspendable call (calls to bounded
/// functions are plain expressions).
/// Functions called anywhere in `e`.
fn callees(e: &Core) -> std::collections::HashSet<u32> {
    let mut out = std::collections::HashSet::new();
    e.walk(&mut |e| {
        if let Core::Call(g, _) = e {
            out.insert(*g);
        }
    });
    out
}

/// Two or more self calls, not all in tail position: recursion that forks.
fn fork_recursive(fid: u32, f: &mithril_front::core::CoreFn) -> bool {
    !f.self_tail_rec && f.body.sum(&mut |e| matches!(e, Core::Call(g, _) if *g == fid) as usize) >= 2
}

pub(crate) fn has_call(e: &Core) -> bool {
    e.any(&mut |e| match e {
        Core::Call(g, _) if !is_bounded(*g) => Some(true),
        Core::App(..) => Some(true),
        // a closure's body is built as a net, not run here; applying one
        // may suspend like a call
        Core::Lam(..) => Some(false),
        _ => None,
    })
}

pub(crate) fn max_var(e: &Core) -> u32 {
    e.max_var()
}

pub(crate) fn free_vars(e: &Core) -> BTreeSet<u32> {
    e.free_vars()
}

// ---- use counting (linear cell discipline) ----

/// Remaining-use counts per variable.
pub(crate) type Cnt = std::collections::HashMap<u32, i64>;

/// Count every variable occurrence in an expression (sum over subtrees).
pub(crate) fn cnt_expr(e: &Core, m: &mut Cnt) {
    e.walk(&mut |e| {
        if let Core::Var(i) = e {
            *m.entry(*i).or_insert(0) += 1;
        }
    });
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
fn shared_classes(bodies: &[Core], tys: &ty::Types) -> std::collections::HashSet<u32> {
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
    out
}

/// The function matches on parameter `p` and, in that match, builds a
/// constructor with the same number of fields as the matched one: owning
/// `p` lets the cell be rebuilt in place (Perceus/Koka: borrowing would
/// trade that reuse for a later teardown by the lender).
fn reuses_param(body: &Core, p: u32, m: &CoreModule, unbox: &std::collections::HashMap<u32, u8>) -> bool {
    let builds = |e: &Core, ar: usize| {
        e.any(&mut |e| match e {
            Core::Ctor(c, _) | Core::Reuse(_, c, _)
                if (*c as usize) < m.ctors.len() && m.ctors[*c as usize].1 == ar && ar > 0 && !unbox.contains_key(c) =>
            {
                Some(true)
            }
            _ => None,
        })
    };
    body.any(&mut |e| match e {
        Core::Match(s, arms) if **s == Core::Var(p) => {
            let hit = arms.iter().any(|(c, _, b)| {
                let ar = m.ctors.get(*c as usize).map(|x| x.1).unwrap_or(0);
                ar > 0 && !unbox.contains_key(c) && builds(b, ar)
            });
            if hit { Some(true) } else { None }
        }
        _ => None,
    })
}

fn borrows(m: &CoreModule, bodies: &[Core], tys: &ty::Types, unbox: &std::collections::HashMap<u32, u8>) -> (Vec<Vec<bool>>, Vec<std::collections::HashSet<u32>>) {
    // Only values of shared datatypes are lent. An int is an immediate, and
    // a linear (never shared) type moves for free and carries no refcount,
    // so a suspended borrower could not take a reference to it. For shared
    // types an escaping read of a borrowed value is an O(1) increment, so a
    // suspension that stores one just takes a reference.
    let shared = shared_classes(bodies, tys);
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
            let esc = escape_mask(&bodies[f], m.fns[f].arity, &bor[f], &bor, &|v| tys.var(f, v) == ty::Ty::Int);
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
/// The lent parameters whose value escapes (stored, returned, passed to an
/// owning parameter); an integer field is an immediate and never does.
fn escape_mask(body: &Core, arity: usize, own_bor: &[bool], bor: &[Vec<bool>], is_int: &dyn Fn(u32) -> bool) -> u64 {
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
    fn walk(e: &Core, mask: &mut HashMap<u32, u64>, esc: &mut u64, bor: &[Vec<bool>], is_int: &dyn Fn(u32) -> bool) {
        match e {
            Core::Let(x, r, b) => {
                if let Core::Var(y) = r.as_ref() {
                    let v = mask.get(y).copied().unwrap_or(0);
                    mask.insert(*x, v);
                }
                walk(r, mask, esc, bor, is_int);
                walk(b, mask, esc, bor, is_int);
            }
            // an array read only looks at its array: not an escape
            Core::Prim(mithril_front::core::Prim::ArrGet | mithril_front::core::Prim::ArrLen, xs) => {
                for (j, x) in xs.iter().enumerate() {
                    if j > 0 {
                        *esc |= var_mask(x, mask);
                    }
                    walk(x, mask, esc, bor, is_int);
                }
            }
            Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                for x in xs {
                    *esc |= var_mask(x, mask);
                    walk(x, mask, esc, bor, is_int);
                }
            }
            Core::Call(g, xs) => {
                for (j, x) in xs.iter().enumerate() {
                    let owned_param =
                        bor.get(*g as usize).map(|ps| !ps.get(j).copied().unwrap_or(false)).unwrap_or(true);
                    if owned_param {
                        *esc |= var_mask(x, mask);
                    }
                    walk(x, mask, esc, bor, is_int);
                }
            }
            Core::Match(s, arms) => {
                let sm = var_mask(s, mask);
                walk(s, mask, esc, bor, is_int);
                for (_, binders, b) in arms {
                    for bv in binders {
                        mask.insert(*bv, if is_int(*bv) { 0 } else { sm });
                    }
                    walk(b, mask, esc, bor, is_int);
                }
            }
            _ => e.kids().into_iter().for_each(|k| walk(k, mask, esc, bor, is_int)),
        }
    }
    walk(body, &mut mask, &mut esc, bor, is_int);
    esc
}

/// Variables derived from borrowed parameters (raw reads, owner upstream).
fn derive_set(body: &Core, own_bor: &[bool]) -> std::collections::HashSet<u32> {
    let mut s: std::collections::HashSet<u32> =
        own_bor.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i as u32).collect();
    fn walk(e: &Core, s: &mut std::collections::HashSet<u32>) {
        match e {
            Core::Let(x, r, b) => {
                if let Core::Var(y) = r.as_ref() {
                    if s.contains(y) {
                        s.insert(*x);
                    }
                }
                walk(r, s);
                walk(b, s);
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
            _ => e.kids().into_iter().for_each(|k| walk(k, s)),
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
use mithril_rt::mithril_core::agents::{list_collect as net_list_collect, ref_entry, ref_head, wire as net_wire};
use mithril_rt::mithril_core::lower::{ClosureSpec, Entry, MatchMeta, NExpr};
use mithril_rt::mithril_core::port::{Port, Tag};
use mithril_rt::mithril_core::rules::{link as net_link, resolve as net_resolve, MatMeta, Prog};

/// The program's tables for the runtime's value helpers (`Tables`).
struct Tb;
impl Tables for Tb {
    #[inline(always)] fn lin(k: u16) -> bool { if k == 0xFFF { LIN_TUP } else { LIN[k as usize] } }
    #[inline(always)] fn unbox_cid(slot: u64) -> u32 { UNBOX_CID[slot as usize] }
    fn dup_closure(ctx: &mut Wctx, p: u64) -> u64 { dup_closure(ctx, p) }
    fn drop_closure(ctx: &mut Wctx, p: u64) {
        net_link(ctx, Port::new(Tag::Era, 0), Port(p));
        let budget = ctx.fuel();
        ctx.reduce_net(pg_ref(), budget);
    }
}
#[inline(always)] fn mk_con(ctx: &mut Wctx, k: u16, fs: &[u64]) -> u64 { mithril_rt::prelude::mk_con::<Tb>(ctx, k, fs) }
#[inline(always)] fn mk_con1(ctx: &mut Wctx, k: u16, f0: u64) -> u64 { mithril_rt::prelude::mk_con1::<Tb>(ctx, k, f0) }
#[inline(always)] fn mk_con2(ctx: &mut Wctx, k: u16, f0: u64, f1: u64) -> u64 { mithril_rt::prelude::mk_con2::<Tb>(ctx, k, f0, f1) }
#[inline(always)] fn mk_con2r(ctx: &mut Wctx, tok: u32, k: u16, f0: u64, f1: u64) -> u64 { mithril_rt::prelude::mk_con2r::<Tb>(ctx, tok, k, f0, f1) }
#[inline(always)] fn consume2k(ctx: &mut Wctx, p: u64, k: u16) -> (u64, u64) { mithril_rt::prelude::consume2k::<Tb>(ctx, p, k) }
#[inline(always)] fn consume2r(ctx: &mut Wctx, p: u64, k: u16) -> (u64, u64, u32) { mithril_rt::prelude::consume2r::<Tb>(ctx, p, k) }
#[inline(always)] fn consume_chain<const N: usize>(ctx: &mut Wctx, p: u64, k: u16) -> [u64; N] { mithril_rt::prelude::consume_chain::<Tb, N>(ctx, p, k) }
#[inline(always)] fn dup_val(ctx: &mut Wctx, p: u64) -> u64 { mithril_rt::prelude::dup_val::<Tb>(ctx, p) }
#[inline(always)] fn free_val(ctx: &mut Wctx, p: u64) { mithril_rt::prelude::free_val::<Tb>(ctx, p) }
#[inline(always)] fn take_field(ctx: &mut Wctx, p: u64, i: usize) -> u64 { mithril_rt::prelude::take_field::<Tb>(ctx, p, i) }
#[inline(always)] fn untup<const K: usize>(ctx: &mut Wctx, p: u64) -> [u64; K] { mithril_rt::prelude::untup::<Tb, K>(ctx, p) }
#[inline(always)] fn arr_new(ctx: &mut Wctx, n: i64, v: u64) -> u64 { mithril_rt::prelude::arr_new::<Tb>(ctx, n, v) }
#[inline(always)] fn arr_get(ctx: &mut Wctx, a: u64, i: i64) -> u64 { mithril_rt::prelude::arr_get::<Tb>(ctx, a, i) }
#[inline(always)] fn arr_set(ctx: &mut Wctx, a: u64, i: i64, v: u64) -> u64 { mithril_rt::prelude::arr_set::<Tb>(ctx, a, i, v) }
#[inline(always)] fn arr_set_i(ctx: &mut Wctx, a: u64, i: i64, v: u64) -> u64 { mithril_rt::prelude::arr_set_i::<Tb>(ctx, a, i, v) }
#[inline(always)] fn arr_own(ctx: &mut Wctx, a: u64) -> u64 { mithril_rt::prelude::arr_own::<Tb>(ctx, a) }
#[inline(always)] fn bin(ctx: &mut Wctx, op: u8, a: u64, b: u64, own: u8) -> u64 { mithril_rt::prelude::bin::<Tb>(ctx, op, a, b, own) }
#[inline(always)] fn cmp(ctx: &mut Wctx, op: u8, a: u64, b: u64, own: u8) -> u64 { mithril_rt::prelude::cmp::<Tb>(ctx, op, a, b, own) }
#[inline(always)] fn zeros(ctx: &mut Wctx, n: usize) -> u64 { mithril_rt::prelude::zeros::<Tb>(ctx, n) }
fn show(eng: &Engine, p: u64) -> String { mithril_rt::prelude::show::<Tb>(eng, p) }

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
            let pg = Pg { fuel: fuel.clamp(1, u32::MAX as i64) as u32, entries: net_entries(), metas: net_metas() };
            let _ = PG.set(pg);
            let mut eng = Engine::new(threads, fuel);
            let root = eng.run(pg_ref(), Redex { a: 0, b: 0, aux: ROOT });
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
