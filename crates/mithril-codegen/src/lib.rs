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

/// Canonical printing of an `eval_core` value; the generated program's
/// `show` produces exactly this for the corresponding runtime value.
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

/// Alpha-rename every binder (Let, match field) of every function to a
/// unique id, so one var id means one value everywhere in the body (the
/// desugarer reuses ids across match arms). Parameters keep 0..arity.
fn uniquify(m: &CoreModule) -> CoreModule {
    type Env = std::collections::HashMap<u32, u32>;
    /// Fresh ids for the binders `xs` over `b`; outer bindings restored after.
    fn scoped(xs: &[u32], b: &Core, env: &mut Env, next: &mut u32) -> (Vec<u32>, Core) {
        let nxs: Vec<u32> = xs.iter().map(|_| { *next += 1; *next - 1 }).collect();
        let saved: Vec<(u32, Option<u32>)> = xs.iter().zip(&nxs).map(|(x, n)| (*x, env.insert(*x, *n))).collect();
        let b2 = go(b, env, next);
        for (x, old) in saved.into_iter().rev() {
            match old {
                Some(v) => env.insert(x, v),
                None => env.remove(&x),
            };
        }
        (nxs, b2)
    }
    fn go(e: &Core, env: &mut Env, next: &mut u32) -> Core {
        match e {
            Core::Var(i) => Core::Var(*env.get(i).unwrap_or(i)),
            Core::Num(_) | Core::Flo(_) => e.clone(),
            Core::Op2(o, a, b) => Core::Op2(o.clone(), Box::new(go(a, env, next)), Box::new(go(b, env, next))),
            Core::Cmp(o, a, b) => Core::Cmp(o.clone(), Box::new(go(a, env, next)), Box::new(go(b, env, next))),
            Core::If(c, t, f) => Core::If(Box::new(go(c, env, next)), Box::new(go(t, env, next)), Box::new(go(f, env, next))),
            Core::Lam(x, b) => {
                let (nx, b2) = scoped(&[*x], b, env, next);
                Core::Lam(nx[0], Box::new(b2))
            }
            Core::App(f, a) => Core::App(Box::new(go(f, env, next)), Box::new(go(a, env, next))),
            Core::Let(x, r, b) => {
                let r2 = go(r, env, next);
                let (nx, b2) = scoped(&[*x], b, env, next);
                Core::Let(nx[0], Box::new(r2), Box::new(b2))
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
                        let (nbs, body2) = scoped(bs, body, env, next);
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
            let mut c = f.body.max_var().max(f.arity as u32) + 1;
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
    let calls: Vec<std::collections::HashSet<u32>> = m.fns.iter().map(|f| callees(&f.body)).collect();
    // parallel sources: forking recursion and proven folds (split across
    // workers by their CALL rule)
    let seed = (0..nf).map(|f| scal[f].is_some() && (fork_recursive(f as u32, &m.fns[f]) || folds[f].is_some())).collect();
    for (f, r) in fixpoint(seed, |f, s| calls[f].iter().any(|g| s[*g as usize])).into_iter().enumerate() {
        if r {
            scal[f] = None;
        }
    }
    scalar::SHIFTED.with(|l| *l.borrow_mut() = scalar::choose_reps(m, &scal));
    scalar::CTX.with(|c| *c.borrow_mut() = scalar::needs_ctx(m, &scal));
    scalar::LEAF.with(|l| *l.borrow_mut() = (0..nf).map(|f| scal[f].is_some() && !any_call(&m.fns[f].body)).collect());
    BOUNDED.with(|b| *b.borrow_mut() = bounded_fns(m).iter().zip(&scal).map(|(x, s)| *x || s.is_some()).collect());
    CLOSURES.with(|c| *c.borrow_mut() = Some(mithril_net::NetProg::new(m)));
    scalar::SIGS.with(|s| *s.borrow_mut() = scal.clone());
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
                shared.borrow_mut().note(&tys.params[fid][i]);
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
    // every IR function, in emission order (a function's forms adjacent)
    let mut fns: Vec<lir::FnDef> = Vec::new();
    let mut emit = |defs: Vec<lir::FnDef>| fns.extend(defs);
    for fid in 0..nf {
        if scal[fid].is_some() {
            // native scalar form + bridging dive form (see scalar.rs)
            emit(scalar::scalar_fn(m, fid as u32, &scal, &bor, true));
        } else {
            emit(seq::dive_fn(m, fid as u32, &bodies[fid], &bor, &bsets[fid], &mut sq, fwd, &unbox, &tys, &iret, &shared));
        }
        if let Some(q) = &fast_code[fid] {
            emit(vec![q.clone()]);
        }
        if let Some((p, c)) = dps[fid] {
            emit(vec![seq::dps_fn(m, fid as u32, p, c, &bodies[fid], &bor, &mut sq, &unbox, &tys, &iret, &shared)]);
        }
        emit(vec![rules::expand_fn(m, fid as u32, &bodies[fid], &bor, &mut sq, &unbox, &tys, &iret, &shared), call_fn(m, fid as u32, folds[fid].as_ref(), join_rule[fid])]);
        if let Some(pf) = &folds[fid] {
            emit(vec![fold::join_fn(fid as u32, pf)]);
        }
    }
    // Segments may enqueue further segments while being emitted.
    let mut done = 0;
    while done < sq.q.len() {
        let seg = sq.q[done].clone();
        done += 1;
        emit(vec![rules::segment_fn(&seg, &bor, &mut sq, &unbox, &tys, &iret, &shared)]);
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
        emit(vec![hole_fn(*id, *cid)]);
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
                    changed |= sh.note(ft);
                }
            }
        }
        ((0..m.ctors.len()).map(|c| !sh.poison && !sh.classes.contains(&tys.class_of[c])).collect(), !sh.poison && !sh.tuples)
    };
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

/// `t` with each `{name}` placeholder replaced by its value.
fn fill(t: &str, vars: &[(&str, String)]) -> String {
    vars.iter().fold(t.to_string(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v))
}

/// The CPU program: `lower` printed by `lir::rust`, plus the Rust glue
/// (`mithril_rt::template`: prelude wrappers, net region, the `Program`
/// impl, `main`) around the program's tables.
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
    out.push_str(mithril_rt::template::PRELUDE);
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
    let vars = [("n_rules", n_rules.to_string()), ("net_rule", net_rule.to_string()), ("diving", diving), ("fire_arms", fire_arms), ("dive_arms", dive_arms)];
    out.push_str(&fill(mithril_rt::template::PROGRAM, &vars));
    out.push_str(mithril_rt::template::MAIN);
    out
}

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
    let nfns = prog.dives.len();
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
    let vars = [("nfns", nfns.to_string()), ("net_rule", prog.net_rule.to_string()), ("fill_rule", prog.fill_rule.to_string()), ("fwd", prog.fwd.to_string())];
    s.push_str(&fill(mithril_rt::template::NET_REGION, &vars));
    s
}

/// The per-function CALL-rule fire arm: unpack (freeing the arg chain),
/// (par-fold split), dive. Arguments arrive owned; on completion the fire
/// reclaims the ones the dive form only borrowed (read-only parameters).
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
    let cs: Vec<_> = m.fns.iter().map(|f| callees(&f.body)).collect();
    fixpoint(vec![false; cs.len()], |f, b| cs[f].iter().all(|g| *g as usize != f && b[*g as usize]))
}

/// Least fixpoint over functions: `set[f]` turns true once `step(f, &set)`
/// holds (updated in place within a round).
pub(crate) fn fixpoint(mut set: Vec<bool>, step: impl Fn(usize, &[bool]) -> bool) -> Vec<bool> {
    loop {
        let mut changed = false;
        for f in 0..set.len() {
            if !set[f] && step(f, &set) {
                set[f] = true;
                changed = true;
            }
        }
        if !changed {
            return set;
        }
    }
}

/// `from` (transitively) calls `to`.
pub(crate) fn reaches(m: &CoreModule, from: u32, to: u32) -> bool {
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
pub(crate) fn inline_attr(body: &Core) -> bool {
    const MAX: usize = 192;
    !any_call(body) && body.size() <= MAX
}

/// `inline_attr` plus: a loop helper desugared from a `while`/`for` with a
/// single call site from another function is that function's own loop, so
/// it always inlines there (without this the backend picks which member of
/// a recursive cycle absorbs the other by accident of ordering).
pub(crate) fn inline_attr_fn(m: &CoreModule, fid: u32) -> bool {
    let f = &m.fns[fid as usize];
    if inline_attr(&f.body) {
        return true;
    }
    if !(f.name.starts_with("__while") || f.name.starts_with("__for")) {
        return false;
    }
    // the caller of each call site from another function
    let sites: Vec<u32> = (0..m.fns.len() as u32)
        .filter(|h| *h != fid)
        .flat_map(|h| std::iter::repeat_n(h, m.fns[h as usize].body.sum(&mut |e| matches!(e, Core::Call(g, _) if *g == fid) as usize)))
        .collect();
    // only on a recursive cycle with its caller (the helper reaches the
    // caller again): there the backend must pick which member absorbs the
    // other, and the source says the loop belongs to its function.
    // Elsewhere the backend's own inlining decision stands.
    matches!(sites[..], [c] if reaches(m, fid, c))
}

/// Register a closure built by compiled code: an entry over its free
/// variables (`caps`, sorted) whose body is the lambda itself.
pub(crate) fn closure_entry(caps: Vec<u32>, lam: &Core) -> u16 {
    CLOSURES.with(|c| c.borrow_mut().as_mut().expect("closure registry").add_entry(caps, lam))
}

pub(crate) fn is_bounded(g: u32) -> bool {
    scalar::flag(&BOUNDED, g, false)
}

/// The callee of every call site in `e` (with repeats), pre-order.
pub(crate) fn call_sites(e: &Core) -> Vec<u32> {
    let mut v = Vec::new();
    e.walk(&mut |e| if let Core::Call(g, _) = e { v.push(*g) });
    v
}

/// Functions called anywhere in `e`.
pub(crate) fn callees(e: &Core) -> std::collections::HashSet<u32> {
    call_sites(e).into_iter().collect()
}

/// Two or more self calls, not all in tail position: recursion that forks.
fn fork_recursive(fid: u32, f: &mithril_front::core::CoreFn) -> bool {
    !f.self_tail_rec && f.body.sum(&mut |e| matches!(e, Core::Call(g, _) if *g == fid) as usize) >= 2
}

/// Whether evaluating `e` may run a suspendable call (calls to bounded
/// functions are plain expressions).
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

/// Use counts of a tail-form body: tail If/Match branches merge by max. In
/// the rule form (`rule`) a let RHS carrying calls is itself a tail form: a
/// call dives inline (continuation runs here) or suspends (continuation
/// moves into a record taking exactly its uses), both consuming the same.
fn cnt_tail(e: &Core, m: &mut Cnt, rule: bool) {
    let arm = |a: &Core| { let mut mm = Cnt::new(); cnt_tail(a, &mut mm, rule); mm };
    match e {
        Core::Let(_, r, b) => {
            if rule && has_call(r) { cnt_tail(r, m, rule) } else { cnt_expr(r, m) }
            cnt_tail(b, m, rule);
        }
        Core::If(c, t, f) => { cnt_expr(c, m); merge_max(m, vec![arm(t), arm(f)]); }
        Core::Match(s, arms) => { cnt_expr(s, m); merge_max(m, arms.iter().map(|(_, _, b)| arm(b)).collect()); }
        other => cnt_expr(other, m),
    }
}
pub(crate) fn cnt_dive(e: &Core, m: &mut Cnt) { cnt_tail(e, m, false) }
pub(crate) fn cnt_rule(e: &Core, m: &mut Cnt) { cnt_tail(e, m, true) }

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
