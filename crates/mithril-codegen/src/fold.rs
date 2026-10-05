//! Proven-fold parallel emission: a fold helper (`__forN`, shape
//! `If(v0 < v1, ..tail self call.., Var acc)`) whose `CoreFn.fold`
//! carries a proven additive combiner gets a chunked par_fold: its CALL rule
//! splits large ranges into a binary fork of net records and spawned CALL
//! redexes (leaf chunks run the dive form under fuel), and a join rule
//! combines the two partial results. Never OS threads: the
//! engine provides the parallelism.
//!
//! Combiners: WrapAdd (mod 2^56), WrapAdd32 (join re-masks to the low 32
//! bits), TupleWrapAdd / TupleWrapAdd32 (elementwise over the accumulator
//! tuple).

use crate::lir::{as_i, bin, c, do_, i64_, let_, num, p, rec_addr, ret, u16_, u32_, u64_, v, Bop, FnDef, Inline, Ty, E, S};
use crate::seq::{dive_args, dive_params, vn};
use mithril_front::core::{Combiner, Core, CoreModule};

pub(crate) struct ParFold {
    /// Parameter index of the accumulator (>= 2; 0 = counter, 1 = bound).
    pub acc: usize,
    /// Join re-masks each element to the low 32 bits (WrapAdd32 modes).
    pub mask32: bool,
    /// Accumulator is an n-tuple (TupleWrapAdd modes).
    pub tuple: Option<usize>,
    /// An index fill: the accumulator is the array being filled. Both halves of
    /// a split write their own index ranges of one buffer, which the right half
    /// borrows from the left without a reference; the join returns the left's.
    pub fill: bool,
    /// Offered to the co-execution channel at its entry: a sum fold with no
    /// native form (its iterations call recursive or forking code, so their
    /// cost is irregular; a native fold runs as a range launch instead).
    pub coop: bool,
}

/// Decide whether `fid` gets the chunked par_fold shape, and find its
/// accumulator parameter. Conservative: any mismatch falls back to the
/// (always sound) generic dive/rule emission.
pub(crate) fn par_fold(m: &CoreModule, fid: u32) -> Option<ParFold> {
    let f = &m.fns[fid as usize];
    let fi = f.fold.as_ref()?;
    if !fi.proven {
        return None;
    }
    let (mask32, tuple) = match &fi.combiner {
        Combiner::WrapAdd | Combiner::Fill => (false, None),
        Combiner::WrapAdd32 => (true, None),
        Combiner::TupleWrapAdd(n) => (false, Some(*n)),
        Combiner::TupleWrapAdd32(n) => (true, Some(*n)),
    };
    let fill = fi.combiner == Combiner::Fill;
    let ar = f.arity;
    if ar < 3 || !f.self_tail_rec {
        return None;
    }
    // Body shape: If(cond, then, Var acc) (the loop's one state variable).
    let (th, el) = match &f.body {
        Core::If(_, t, e) => (t, e),
        _ => return None,
    };
    let acc = match el.as_ref() {
        Core::Var(k) if (*k as usize) >= 2 && (*k as usize) < ar => *k as usize,
        _ => return None,
    };
    // Every self call must pass the bound and all non-acc extras through.
    let mut calls: Vec<Vec<Core>> = Vec::new();
    th.walk(&mut |e| {
        if let Core::Call(g, a) = e {
            if *g == fid {
                calls.push(a.clone());
            }
        }
    });
    if calls.is_empty() {
        return None;
    }
    for args in calls {
        if args.len() != ar || args[1] != Core::Var(1) {
            return None;
        }
        for (j, a) in args.iter().enumerate().skip(2) {
            if j != acc && *a != Core::Var(j as u32) {
                return None;
            }
        }
    }
    Some(ParFold { acc, mask32, tuple, fill, coop: false })
}

/// The range-split preamble of the fold fn's CALL rule (before the dive).
pub(crate) fn split_snippet(fid: u32, ar: usize, pf: &ParFold, join_rule: u16) -> Vec<S> {
    let mut code = coop_snippet(fid, ar, pf, join_rule, Some(v("parent")));
    code.extend(split_code(fid, ar, pf, join_rule, &[], v("parent"), crate::lir::ret_unit()));
    code
}

/// The same split inside the fold's dive, at each loop head: the remaining
/// range [v0, v1) splits once its measured work exceeds a budget; the dive
/// suspends to the join record, whose parent the caller attaches (the
/// accumulator so far rides in the left half). `bor` are the parameters the
/// function borrows from its caller.
pub(crate) fn split_snippet_dive(fid: u32, ar: usize, pf: &ParFold, join_rule: u16, bor: &[bool]) -> Vec<S> {
    let mut code = coop_snippet(fid, ar, pf, join_rule, None);
    code.extend(split_code(fid, ar, pf, join_rule, bor, E::Const("NONE".into()), ret(crate::lir::err(crate::lir::cast(v("j"), Ty::U64)))));
    code
}

fn split_code(fid: u32, ar: usize, pf: &ParFold, join_rule: u16, bor: &[bool], parent: E, exit: S) -> Vec<S> {
    let rc = 1 + fid;
    let zline = match pf.tuple {
        None => let_("tz", Ty::U64, num(i64_(0))),
        Some(n) => let_("tz", Ty::U64, c("zeros", vec![crate::lir::usize_(n)])),
    };
    // The left chunk takes the original extras, or a reference to a borrowed
    // one (the caller keeps its own); the right chunk gets copies. Both spawned
    // calls own their arguments.
    let mut dups = Vec::new();
    let left: Vec<E> = (0..ar)
        .map(|i| {
            if i == 1 {
                v("tmid")
            } else if i != 0 && i != pf.acc && bor.get(i).copied().unwrap_or(false) {
                dups.push(let_(format!("tl{i}"), Ty::U64, c("dup_val", vec![v(vn(i as u32))])));
                v(format!("tl{i}"))
            } else {
                v(vn(i as u32))
            }
        })
        .collect();
    let right: Vec<E> = (0..ar)
        .map(|i| {
            if i == 0 {
                v("tmid")
            } else if i == 1 {
                v("v1")
            } else if i == pf.acc {
                v(if pf.fill { vn(i as u32) } else { "tz".into() })
            } else {
                dups.push(let_(format!("td{i}"), Ty::U64, c("dup_val", vec![v(vn(i as u32))])));
                v(format!("td{i}"))
            }
        })
        .collect();
    // Split by work, not iterations: FOLD_EST_<fid> is the measured fuel
    // per iteration (set when a chunk's dive runs out of fuel; 1 until
    // then), so a range splits while its estimated work exceeds one budget
    // estimates are work units; one budget is one work quantum (on the CPU the
    // budget itself, on the device the work after which a dive yields)
    let budget = p("imax", vec![c("work_quantum", vec![]), i64_(256)]);
    let mut inner = vec![let_("mid", Ty::I64, bin(Bop::Add, v("lo"), bin(Bop::Div, bin(Bop::Sub, v("hi"), v("lo")), i64_(2)))), let_("tmid", Ty::U64, num(v("mid"))), zline];
    inner.extend(dups);
    inner.push(let_("j", Ty::U32, c("alloc_rec", vec![u16_(join_rule as u64), u32_(2), u32_(0), u32_(0), parent])));
    inner.push(do_(c("spawn_call", vec![u16_(rc as u64), E::Slice(left), rec_addr("j")])));
    inner.push(do_(c("spawn_call", vec![u16_(rc as u64), E::Slice(right), bin(Bop::Or, rec_addr("j"), u64_(1))])));
    inner.push(exit);
    vec![
        S::Comment("par-fold split: proven fold, chunked binary fork over [v0, v1)".into()),
        let_("lo", Ty::I64, as_i(v("v0"))),
        let_("hi", Ty::I64, as_i(v("v1"))),
        let_("est", Ty::I64, p("imax", vec![p("atomic_load", vec![E::Addr(format!("FOLD_EST_{fid}"))]), i64_(1)])),
        S::If(
            bin(Bop::Ge, bin(Bop::Sub, v("hi"), v("lo")), i64_(2)),
            vec![S::If(bin(Bop::Gt, p("sat_mul", vec![bin(Bop::Sub, v("hi"), v("lo")), v("est")]), budget), inner, vec![])],
            vec![],
        ),
    ]
}

/// Work above which a fold's range runs as one device-wide pass instead
/// of chunk tasks: about 2^16 units is tens of microseconds on one lane,
/// the order of a launch round trip (heat2d at width 256: one lane took
/// 25 ms per 65 K-cell step; the range kernel takes 8 us at 1 M cells).
const RANGE_MIN: i64 = 1 << 16;

/// The fold's range launch (device): the request owns a reference to every
/// argument (a borrowed one is shared, an owned one moves), the bridge
/// suspends to a record of the join rule, and the completion delivers to
/// both of its slots (a fill's buffer twice; a sum and the incoming
/// accumulator). `kind`: 0 fill, 1 sum, 2 sum mod 2^32 (`RangeFill`).
pub(crate) fn range_snippet(fid: u32, ar: usize, pf: &ParFold, join_rule: u16, bor: &[bool], kind: u32) -> Vec<S> {
    let mut launch = Vec::new();
    let ports: Vec<E> = (0..ar)
        .map(|i| {
            if i >= 2 && i != pf.acc && bor.get(i).copied().unwrap_or(false) {
                launch.push(let_(format!("tr{i}"), Ty::U64, c("dup_val", vec![v(vn(i as u32))])));
                v(format!("tr{i}"))
            } else {
                v(vn(i as u32))
            }
        })
        .collect();
    let (lo, hi) = (as_i(v("v0")), as_i(v("v1")));
    launch.push(let_("rj", Ty::U32, c("range_launch", vec![u32_(fid as u64), u16_(join_rule as u64), lo.clone(), hi.clone(), E::Slice(ports), u32_(pf.acc as u64), u32_(kind as u64)])));
    launch.push(ret(crate::lir::err(crate::lir::cast(v("rj"), Ty::U64))));
    let est = p("imax", vec![p("atomic_load", vec![E::Addr(format!("FOLD_EST_{fid}"))]), i64_(1)]);
    let work = p("sat_mul", vec![bin(Bop::Sub, hi, lo), est]);
    vec![S::Comment("range launch: the fold runs as one device-wide pass".into()), S::If(bin(Bop::Gt, work, i64_(RANGE_MIN)), launch, vec![])]
}

/// Iterations from which a fold is offered to the co-execution channel. The
/// offer is decided by the range alone, at the fold's entry, so every engine
/// running the program makes the same offer for the same range (a measured
/// cost would be known at different points on different engines, and their
/// keys would differ).
const COOP_MIN_LEN: i64 = 64;

/// A sum fold's offer to the co-execution channel (`coop_launch`), ahead of
/// its split. Taken, the fold suspends to the returned join record: a dive
/// returns it, a CALL rule (`parent`) attaches its parent to it. The ports
/// are the fold's own: an offer is declined unless every argument is an
/// int, so none needs a reference. Fills and tuple folds are not offered.
fn coop_snippet(fid: u32, ar: usize, pf: &ParFold, join_rule: u16, parent: Option<E>) -> Vec<S> {
    if !pf.coop || pf.fill || pf.tuple.is_some() {
        return Vec::new();
    }
    let kind = if pf.mask32 { 2 } else { 1 };
    let ports: Vec<E> = (0..ar).map(|i| v(vn(i as u32))).collect();
    let (lo, hi) = (as_i(v("v0")), as_i(v("v1")));
    let offer = let_("cj", Ty::U32, c("coop_launch", vec![u32_(fid as u64), u16_(join_rule as u64), lo.clone(), hi.clone(), E::Slice(ports), u32_(pf.acc as u64), u32_(kind)]));
    // a CALL rule is an entry; a dive offers at its first loop head only
    let (suspend, entry) = match parent {
        Some(parent) => (vec![do_(c("set_parent", vec![v("cj"), parent])), crate::lir::ret_unit()], None),
        None => (vec![ret(crate::lir::err(crate::lir::cast(v("cj"), Ty::U64)))], Some(bin(Bop::Eq, as_i(v("v0")), as_i(v("fold_start"))))),
    };
    let taken = S::If(bin(Bop::Ne, v("cj"), u32_(0)), suspend, vec![]);
    let long = bin(Bop::Ge, bin(Bop::Sub, hi, lo), i64_(COOP_MIN_LEN));
    let mut offer = vec![S::If(long, vec![offer, taken], vec![])];
    if let Some(entry) = entry {
        offer = vec![S::If(entry, offer, vec![])];
    }
    let mut code = vec![S::Comment("co-execution: a long irregular sum fold is offered at its entry".into())];
    code.extend(offer);
    code
}

/// The native bridge's first measurement. A native chunk never runs out of
/// budget, so the dive's measurement never fires for it: while the fold has no
/// estimate, the bridge runs one iteration natively, measures its work, and
/// continues on the range after it, where the split sees the estimate. The
/// estimate is in work units, the same on both backends. A borrowed parameter is
/// lent to both calls; an owned one gives the probe a reference.
pub(crate) fn probe_snippet(fid: u32, ar: usize, pf: &ParFold, bor: &[bool]) -> Vec<S> {
    let mut body = vec![let_("f0", Ty::I64, p("work_mark", vec![v("fuel")])), let_("tp", Ty::U64, num(bin(Bop::Add, as_i(v("v0")), i64_(1))))];
    let mut probe = vec![v("fuel")];
    for i in 0..ar {
        probe.push(if i == 1 {
            v("tp")
        } else if i == 0 || i == pf.acc || bor.get(i).copied().unwrap_or(false) {
            v(vn(i as u32))
        } else {
            body.push(let_(format!("tq{i}"), Ty::U64, c("dup_val", vec![v(vn(i as u32))])));
            v(format!("tq{i}"))
        });
    }
    let rest: Vec<E> = std::iter::once(v("fuel")).chain((0..ar).map(|i| match i {
        0 => v("tp"),
        _ if i == pf.acc => v("pa"),
        _ => v(vn(i as u32)),
    })).collect();
    // the work one iteration takes
    let used = p("work_since", vec![v("fuel"), v("f0")]);
    let est = used;
    body.push(S::Res(
        E::Call { f: format!("d_{fid}"), ctx: true, args: probe },
        "pa".into(),
        vec![
            do_(p("atomic_store", vec![E::Addr(format!("FOLD_EST_{fid}")), p("imax", vec![est, i64_(1)])])),
            do_(p("flag_store", vec![E::Addr(format!("FOLD_MEAS_{fid}")), E::Bool(true)])),
            ret(E::Call { f: format!("d_{fid}"), ctx: true, args: rest }),
        ],
        "pe".into(),
        vec![S::Unreachable],
    ));
    let unmeasured = E::Not(Box::new(p("flag_load", vec![E::Addr(format!("FOLD_MEAS_{fid}"))])));
    let long = bin(Bop::Ge, bin(Bop::Sub, as_i(v("v1")), as_i(v("v0"))), i64_(2));
    vec![S::If(unmeasured, vec![S::If(long, body, vec![])], vec![])]
}

/// The per-fold work estimate and the dive-side measurement: at a fuel-out
/// the chunk has run `v0 - fold_start` iterations on one budget.
pub(crate) fn est_static(fid: u32) -> String {
    format!(
        "static FOLD_EST_{fid}: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);\nstatic FOLD_MEAS_{fid}: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);\n"
    )
}

/// The fold dive's entry: a dive that suspended inside a callee before any
/// loop-head measurement ran more than a budget within one iteration, so
/// its iterations are at least that heavy.
pub(crate) fn heavy_wrapper(fid: u32, ar: usize) -> FnDef {
    let body = vec![
        let_("r", Ty::Res, E::Call { f: format!("dd_{fid}"), ctx: true, args: dive_args(ar) }),
        S::If(
            p("is_err", vec![E::Addr("r".into())]),
            vec![S::If(E::Not(Box::new(p("flag_load", vec![E::Addr(format!("FOLD_MEAS_{fid}"))]))), vec![do_(p("atomic_max", vec![E::Addr(format!("FOLD_EST_{fid}")), c("work_quantum", vec![])]))], vec![])],
            vec![],
        ),
        ret(v("r")),
    ];
    FnDef { name: format!("d_{fid}"), ctx: true, params: dive_params(ar), ret: Ty::Res, body, inline: Inline::Default, cold: false }
}

pub(crate) fn est_update(fid: u32) -> Vec<S> {
    vec![
        let_("it", Ty::I64, p("imax", vec![bin(Bop::Sub, as_i(v("v0")), as_i(v("fold_start"))), i64_(1)])),
        do_(p("atomic_store", vec![E::Addr(format!("FOLD_EST_{fid}")), p("imax", vec![bin(Bop::Div, c("work_quantum", vec![]), v("it")), i64_(1)])])),
        do_(p("flag_store", vec![E::Addr(format!("FOLD_MEAS_{fid}")), E::Bool(true)])),
    ]
}

/// The fold's join rule: combines the two partial results (each the
/// loop's accumulator: an int, or an int tuple added elementwise in
/// place into the left one, the right one freed) and delivers the sum.
pub(crate) fn join_fn(fid: u32, pf: &ParFold) -> FnDef {
    let combine = match pf.tuple {
        // both halves filled one buffer; the right's value is the borrowed alias
        _ if pf.fill => vec![let_("r", Ty::U64, v("a"))],
        None => {
            let sum = bin(Bop::Add, v("x"), v("y"));
            let r = if pf.mask32 { bin(Bop::And, sum, i64_(0xFFFF_FFFF)) } else { sum };
            vec![let_("x", Ty::I64, p("take_i", vec![v("a")])), let_("y", Ty::I64, p("take_i", vec![v("b")])), let_("r", Ty::U64, num(r))]
        }
        Some(_) => vec![let_("r", Ty::U64, c("tup_add", vec![v("a"), v("b"), E::Bool(pf.mask32)]))],
    };
    let mut body = vec![let_("parent", Ty::U64, c("rec_parent", vec![crate::lir::cast(v("aux"), Ty::U32)]))];
    body.extend(combine);
    body.push(do_(c("deliver", vec![v("parent"), v("r")])));
    FnDef { name: format!("jn_{fid}"), ctx: true, params: crate::rules::rule_params(), ret: Ty::Unit, body, inline: Inline::Default, cold: false }
}
