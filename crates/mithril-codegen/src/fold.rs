//! Proven-fold parallel emission: a fold helper (`__forN`, shape
//! `If(v0 < v1, ..tail self call.., Tuple([Var acc]))`) whose `CoreFn.fold`
//! carries a proven additive combiner gets a chunked par_fold: its CALL rule
//! splits large ranges into a binary fork of net records and spawned CALL
//! redexes (leaf chunks run the dive form under fuel), and a join rule
//! combines the two 1-tuple partial results in place. Never OS threads: the
//! engine provides the parallelism.
//!
//! Combiners: WrapAdd (mod 2^56), WrapAdd32 (join re-masks to the low 32
//! bits), TupleWrapAdd / TupleWrapAdd32 (elementwise over the accumulator
//! tuple). `Fn` combiners are not parallelized (sequential fallback is
//! always sound).

use mithril_front::core::{Combiner, Core, CoreModule};

pub(crate) struct ParFold {
    /// Parameter index of the accumulator (>= 2; 0 = counter, 1 = bound).
    pub acc: usize,
    /// Join re-masks each element to the low 32 bits (WrapAdd32 modes).
    pub mask32: bool,
    /// Accumulator is an n-tuple (TupleWrapAdd modes).
    pub tuple: Option<usize>,
}

fn collect_self_calls<'e>(e: &'e Core, fid: u32, out: &mut Vec<&'e Vec<Core>>) {
    match e {
        Core::Call(g, args) => {
            if *g == fid {
                out.push(args);
            }
            for a in args {
                collect_self_calls(a, fid, out);
            }
        }
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            collect_self_calls(a, fid, out);
            collect_self_calls(b, fid, out);
        }
        Core::If(a, b, c) => {
            collect_self_calls(a, fid, out);
            collect_self_calls(b, fid, out);
            collect_self_calls(c, fid, out);
        }
        Core::Let(_, r, b) => {
            collect_self_calls(r, fid, out);
            collect_self_calls(b, fid, out);
        }
        Core::Ctor(_, xs) | Core::Tuple(xs) => {
            for x in xs {
                collect_self_calls(x, fid, out);
            }
        }
        Core::Match(s, arms) => {
            collect_self_calls(s, fid, out);
            for (_, _, b) in arms {
                collect_self_calls(b, fid, out);
            }
        }
        Core::Proj(a, _) => collect_self_calls(a, fid, out),
    }
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
        Combiner::WrapAdd => (false, None),
        Combiner::WrapAdd32 => (true, None),
        Combiner::TupleWrapAdd(n) => (false, Some(*n)),
        Combiner::TupleWrapAdd32(n) => (true, Some(*n)),
        Combiner::Fn(_) => return None,
    };
    let ar = f.arity;
    if ar < 3 || !f.self_tail_rec {
        return None;
    }
    // Body shape: If(cond, then, Tuple([Var acc])).
    let (th, el) = match &f.body {
        Core::If(_, t, e) => (t, e),
        _ => return None,
    };
    let acc = match el.as_ref() {
        Core::Tuple(items) if items.len() == 1 => match &items[0] {
            Core::Var(k) if (*k as usize) >= 2 && (*k as usize) < ar => *k as usize,
            _ => return None,
        },
        _ => return None,
    };
    // Every self call must pass the bound and all non-acc extras through.
    let mut calls = Vec::new();
    collect_self_calls(th, fid, &mut calls);
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
    Some(ParFold { acc, mask32, tuple })
}

/// The range-split preamble of the fold fn's CALL rule (before the dive).
pub(crate) fn split_snippet(fid: u32, ar: usize, pf: &ParFold, join_rule: u16) -> String {
    let rc = 1 + fid;
    let zline = match pf.tuple {
        None => "let tz = num(0i64);".to_string(),
        Some(n) => format!("let tz = zeros(ctx, {n});"),
    };
    let left: Vec<String> = (0..ar)
        .map(|i| match i {
            0 => "v0".into(),
            1 => "tmid".into(),
            _ => format!("v{i}"),
        })
        .collect();
    let right: Vec<String> = (0..ar)
        .map(|i| {
            if i == 0 {
                "tmid".into()
            } else if i == 1 {
                "v1".into()
            } else if i == pf.acc {
                "tz".into()
            } else {
                format!("v{i}")
            }
        })
        .collect();
    format!(
        "// par-fold split: proven fold, chunked binary fork over [v0, v1)\n{{\nlet lo = as_i(v0);\nlet hi = as_i(v1);\nif hi - lo > ctx.fuel().max(2) {{\nlet mid = lo + (hi - lo) / 2;\nlet tmid = num(mid);\n{zline}\nlet j = ctx.alloc_rec({join_rule}u16, 2, 0, 0, parent);\nspawn_call(ctx, {rc}u16, &[{}], (j as u64) << 3);\nspawn_call(ctx, {rc}u16, &[{}], ((j as u64) << 3) | 1);\nreturn;\n}}\n}}\n",
        left.join(", "),
        right.join(", ")
    )
}

/// The fold's join rule: combines the two 1-tuple partial results (each is
/// `Tuple([acc])`) in place into the left one and delivers it.
pub(crate) fn join_fn(fid: u32, pf: &ParFold) -> String {
    let combine = match pf.tuple {
        None => {
            let s = if pf.mask32 {
                "x.wrapping_add(y) & 0xFFFF_FFFF".to_string()
            } else {
                "wrap56(x.wrapping_add(y))".to_string()
            };
            format!(
                "let x = as_i(ctx.cell(ca)[0]);\nlet y = as_i(ctx.cell(cb)[0]);\nctx.set(ca, 0, num({s}));\n"
            )
        }
        Some(_) => format!(
            "let ta = ctx.cell(ca)[0];\nlet tb = ctx.cell(cb)[0];\nlet s = tup_add(ctx, ta, tb, {});\nctx.set(ca, 0, s);\n",
            pf.mask32
        ),
    };
    format!(
        "fn jn_{fid}(ctx: &mut Wctx, e: Redex) {{\nlet parent = ctx.rec(e.aux as u32).parent;\nlet ca = con_addr(e.a);\nlet cb = con_addr(e.b);\n{combine}ctx.deliver(parent, e.a);\n}}\n\n"
    )
}
