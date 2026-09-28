//! Native scalar lowering.
//!
//! A function is *scalar* when every value it computes is an i56 integer:
//! `Num / Var / Op2 / Cmp / If / Let`, calls to other scalar functions, and —
//! because `while`/`for` desugar into helpers that tail-return a `Tuple` of
//! the live loop variables — an all-integer `Tuple` in tail position
//! (kind `SK(k)`), consumed at call sites exclusively through the
//! `Let(t, Call(g), .. Proj(Var t, i) ..)` idiom the desugarer emits.
//! No floats, constructors, matches, or escaping tuples.
//!
//! Scalar functions are emitted as plain `fn s_<fid>(v0: i64, ..) -> i64`
//! (or `-> (i64, .., i64)` for `SK(k)`) with native wrapping arithmetic (one
//! `wrap56` sign-fix per op, exactly matching `bin`'s int path),
//! self-tail-recursion as a loop, and no ctx/fuel/ownership plumbing. Their
//! `d_<fid>` dive form becomes a thin bridge (unpack ports -> call `s_` ->
//! repack; `SK` results build the same 0xFFF tuple cell the dive form
//! produced), so every existing call site — dive calls, CALL rules, fold
//! leaves — takes the fast path unchanged. Scalar calls consume no fuel:
//! they terminate by their own data-driven control flow, so a dive cannot
//! suspend inside one (suspension granularity coarsens by at most one scalar
//! call's work).
//!
//! Type-consistency note: the classifier proves int-ness from the callee's
//! side; a source program passing a non-NUM port into a scalar function gets
//! garbage arithmetic rather than a type error — identical to the
//! pre-lowering behavior of `bin`'s unchecked int path, and pinned by the
//! checksum oracles in every test lane.

use crate::lir::{as_i, bin, burn_fuel, c, cast, free, i64_, let_, num, ok, p, ret, set, u16_, usize_, v, Bop, FnDef, Inline, Pat, Ty, E, S};
use crate::seq::{arith, bin_code, cmp_code, compare, vn, vparams};
use mithril_front::core::{Core, CoreModule};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Kind {
    No,
    S1,
    SK(usize),
}

/// Scalar parameter type: a bare i64, a k-tuple of i64s passed as k
/// native components (read only through constant `Proj`), or an int array
/// (its port bits in an i64) the function owns (`A`: consumed, or freed at
/// the end of each path) or borrows (`B`: only read; the caller keeps it).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum PTy {
    I,
    T(usize),
    A,
    B,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Sig {
    pub params: Vec<PTy>,
    pub ret: Kind, // S1 or SK(k); No never appears inside a Some(Sig)
    /// which return components are (owned) int arrays: one entry for S1,
    /// k for SK(k)
    pub ra: Vec<bool>,
}

/// An owned array slot: a variable, or component `i` of a destructured
/// tuple variable `t`.
fn slot_key(e: &Core, tarr: &HashMap<u32, Vec<bool>>, arrs: &HashMap<u32, bool>) -> Option<u64> {
    match e {
        Core::Var(v) if arrs.contains_key(v) => Some(*v as u64),
        Core::Proj(b, i) => match &**b {
            Core::Var(t) if tarr.get(t).is_some_and(|m| m.get(*i).copied().unwrap_or(false)) => {
                Some((1u64 << 40) | ((*t as u64) << 8) | *i as u64)
            }
            _ => None,
        },
        _ => None,
    }
}

fn slot_name(key: u64) -> String {
    if key >> 40 != 0 {
        format!("q{}_{}", (key >> 8) & 0xFFFF_FFFF, key & 0xFF)
    } else {
        format!("v{key}")
    }
}

/// Whether `e` is array-valued (under the current bindings).
fn akind(sigs: &[Option<Sig>], tarr: &HashMap<u32, Vec<bool>>, arrs: &HashMap<u32, bool>, e: &Core) -> bool {
    use mithril_front::core::Prim;
    match e {
        Core::Var(v) => arrs.contains_key(v),
        Core::Proj(..) => slot_key(e, tarr, arrs).is_some(),
        Core::Prim(Prim::ArrNew | Prim::ArrSet, _) => true,
        Core::Call(g, _) => matches!(&sigs[*g as usize], Some(s) if s.ret == Kind::S1 && s.ra.first() == Some(&true)),
        Core::If(_, x, _) => akind(sigs, tarr, arrs, x),
        Core::Let(_, _, b) => akind(sigs, tarr, arrs, b),
        _ => false,
    }
}

/// Array mask of a tuple-valued expression of `k` components.
fn tmask(sigs: &[Option<Sig>], tarr: &HashMap<u32, Vec<bool>>, arrs: &HashMap<u32, bool>, e: &Core, k: usize) -> Vec<bool> {
    match e {
        Core::Tuple(items) => items.iter().map(|it| akind(sigs, tarr, arrs, it)).collect(),
        Core::If(_, x, _) => tmask(sigs, tarr, arrs, x, k),
        Core::Let(_, _, b) => tmask(sigs, tarr, arrs, b, k),
        Core::Call(g, _) => sigs[*g as usize].as_ref().map(|s| s.ra.clone()).unwrap_or_else(|| vec![false; k]),
        _ => vec![false; k],
    }
}

struct Chk<'m> {
    why: Option<String>,
    sigs: &'m [Option<Sig>],
    /// component vars: SK-destructured lets AND T(k) params -> arity
    tvars: HashMap<u32, usize>,
    ok: bool,
    /// array vars -> borrowed (true) or owned
    arrs: HashMap<u32, bool>,
    /// tuple vars -> which components are arrays
    tarr: HashMap<u32, Vec<bool>>,
    /// owned array slots alive on the current path
    live: std::collections::BTreeSet<u64>,
    /// borrowed params this body consumes (they must become owned)
    demote: Vec<u32>,
    /// borrowed alias -> the parameter it names
    root: HashMap<u32, u32>,
}

impl<'m> Chk<'m> {
    fn fail(&mut self, e: &Core) {
        if self.why.is_none() {
            self.why = Some(format!("{:.160}", format!("{:?}", e)));
        }
        self.ok = false;
    }

    fn akind(&self, e: &Core) -> bool {
        akind(self.sigs, &self.tarr, &self.arrs, e)
    }

    fn slot(&self, e: &Core) -> Option<u64> {
        slot_key(e, &self.tarr, &self.arrs)
    }

    /// An array read in place: must name a live slot or a borrowed var.
    /// (Every array in native code holds ints: parameters are typed
    /// `Arr(true)` at the boundary, and inside, arrays are only built and
    /// written with int-checked values.)
    fn read(&mut self, e: &Core) {
        match e {
            Core::Var(v) if self.arrs.get(v) == Some(&true) => {}
            _ => match self.slot(e) {
                Some(k) if self.live.contains(&k) => {}
                _ => self.fail(e),
            },
        }
    }

    /// Consume slot `k` (an owned array moves on).
    fn consume(&mut self, e: &Core, k: u64) {
        if let Core::Var(v) = e {
            if self.arrs.get(v) == Some(&true) {
                // a borrowed param handed on as owned: it must be owned
                self.demote.push(self.root.get(v).copied().unwrap_or(*v));
                return self.fail(e);
            }
        }
        if !self.live.remove(&k) {
            self.fail(e); // consumed twice on this path
        }
    }

    /// An array-valued expression in a consuming position. A slot operand
    /// is returned for deferred consumption (it moves when the enclosing
    /// operation runs, after its other operands are evaluated).
    fn aexpr(&mut self, e: &Core) -> Option<u64> {
        use mithril_front::core::Prim;
        if !self.ok {
            return None;
        }
        if let Some(k) = self.slot(e) {
            return Some(k);
        }
        match e {
            Core::Var(v) if self.arrs.get(v) == Some(&true) => {
                self.demote.push(self.root.get(v).copied().unwrap_or(*v));
                self.fail(e);
            }
            Core::Prim(Prim::ArrNew, xs) => {
                self.expr(&xs[0]);
                self.expr(&xs[1]);
            }
            Core::Prim(Prim::ArrSet, xs) => {
                let d = self.aexpr(&xs[0]);
                self.expr(&xs[1]);
                self.expr(&xs[2]);
                if let Some(k) = d {
                    self.consume(&xs[0], k);
                }
            }
            Core::Call(g, args) => match &self.sigs[*g as usize] {
                Some(sig) if sig.ret == Kind::S1 && sig.ra.first() == Some(&true) => self.args(sig.params.clone(), args),
                _ => self.fail(e),
            },
            Core::If(c, x, y) => {
                self.expr(c);
                let before = self.live.clone();
                let d = self.aexpr(x);
                self.settle(x, d);
                let after = std::mem::replace(&mut self.live, before);
                let d = self.aexpr(y);
                self.settle(y, d);
                if self.live != after {
                    self.fail(e);
                }
            }
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                let d = self.aexpr(b);
                self.settle(b, d);
            }
            _ => self.fail(e),
        }
        None
    }

    /// Consume a deferred slot now.
    fn settle(&mut self, e: &Core, d: Option<u64>) {
        if let Some(k) = d {
            self.consume(e, k);
        }
    }

    /// A value in a consuming position that may be an int or an array:
    /// returns the deferred slot, if any.
    fn any(&mut self, e: &Core) -> Option<u64> {
        if self.akind(e) {
            self.aexpr(e)
        } else {
            self.expr(e);
            None
        }
    }

    /// `e` is a plain i64-valued expression.
    fn expr(&mut self, e: &Core) {
        if !self.ok {
            return;
        }
        match e {
            Core::Num(_) => {}
            Core::Prim(mithril_front::core::Prim::ArrGet, xs) => {
                self.read(&xs[0]);
                self.expr(&xs[1]);
            }
            Core::Prim(mithril_front::core::Prim::ArrLen, xs) => self.read(&xs[0]),
            Core::Prim(p, xs) if p.is_f32() => xs.iter().for_each(|x| self.expr(x)),
            Core::Var(i) if self.arrs.contains_key(i) => self.fail(e),
            Core::Var(i) => {
                if self.tvars.contains_key(i) {
                    { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", e))); } self.ok = false }; // tuple var escaping without Proj
                }
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                self.expr(a);
                self.expr(b);
            }
            Core::If(c, x, y) => {
                self.expr(c);
                let before = self.live.clone();
                self.expr(x);
                let after = std::mem::replace(&mut self.live, before);
                self.expr(y);
                if self.live != after {
                    self.fail(e);
                }
            }
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.expr(b);
            }
            Core::Call(g, args) => {
                match &self.sigs[*g as usize] {
                    Some(sig) if sig.ret == Kind::S1 && sig.ra.first() != Some(&true) => self.args(sig.params.clone(), args),
                    _ => { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", e))); } self.ok = false },
                }
            }
            Core::Proj(..) if self.slot(e).is_some() => self.fail(e), // an array component
            Core::Proj(b, i) => match &**b {
                Core::Var(t) if self.tvars.get(t).is_some_and(|k| i < k) => {}
                _ => { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", e))); } self.ok = false },
            },
            _ => { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", e))); } self.ok = false },
        }
    }

    /// Check call arguments against the callee's parameter types.
    fn args(&mut self, ptys: Vec<PTy>, args: &[Core]) {
        let mut moved: Vec<(usize, u64)> = Vec::new();
        let mut lent: Vec<u64> = Vec::new();
        for (j, (pt, a)) in ptys.iter().zip(args).enumerate() {
            match pt {
                PTy::A => {
                    if let Some(k) = self.aexpr(a) {
                        moved.push((j, k));
                    }
                }
                PTy::B => {
                    self.read(a);
                    lent.extend(self.slot(a));
                }
                PTy::I if self.akind(a) => self.fail(a),
                PTy::I => self.expr(a),
                PTy::T(k) => match a {
                    Core::Var(t) if self.tvars.get(t) == Some(k) => {}
                    Core::Tuple(items) if items.len() == *k => {
                        for it in items {
                            self.expr(it);
                        }
                    }
                    _ => { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", a))); } self.ok = false },
                },
            }
        }
        // an array lent to the callee must not also move into it: the
        // callee would read what it writes
        if moved.iter().any(|(_, k)| lent.contains(k)) {
            return self.fail(&Core::Tuple(args.to_vec()));
        }
        for (j, k) in moved {
            self.consume(&args[j], k);
        }
    }

    /// A let binding: a plain scalar RHS, the SK-destructure idiom, or a
    /// tuple-valued expression (a join point: `if`/`match` arms that each
    /// yield a tuple of the variables they assign) bound as components.
    fn bind(&mut self, x: u32, r: &Core) {
        if let Some(k) = tuple_kind(self.sigs, r) {
            let mask = tmask(self.sigs, &self.tarr, &self.arrs, r, k);
            self.tuple_expr(r, k);
            self.tvars.insert(x, k);
            for (i, a) in mask.iter().enumerate() {
                if *a {
                    self.live.insert((1u64 << 40) | ((x as u64) << 8) | i as u64);
                }
            }
            self.tarr.insert(x, mask);
            return;
        }
        if self.akind(r) {
            if let Core::Var(v) = r {
                if self.arrs.get(v) == Some(&true) {
                    self.arrs.insert(x, true); // alias of a borrowed array
                    let r = self.root.get(v).copied().unwrap_or(*v);
                    self.root.insert(x, r);
                    return;
                }
            }
            let d = self.aexpr(r);
            self.settle(r, d);
            self.arrs.insert(x, false);
            self.live.insert(x as u64);
            return;
        }
        self.expr(r);
    }

    /// A tuple-valued expression of `k` components.
    fn tuple_expr(&mut self, e: &Core, k: usize) {
        if !self.ok {
            return;
        }
        match e {
            Core::Tuple(items) if items.len() == k => {
                let moved: Vec<(usize, u64)> =
                    items.iter().enumerate().filter_map(|(j, it)| self.any(it).map(|d| (j, d))).collect();
                for (j, d) in moved {
                    self.consume(&items[j], d);
                }
            }
            Core::If(c, x, y) => {
                self.expr(c);
                let before = self.live.clone();
                self.tuple_expr(x, k);
                let after = std::mem::replace(&mut self.live, before);
                self.tuple_expr(y, k);
                if self.live != after || tmask(self.sigs, &self.tarr, &self.arrs, x, k) != tmask(self.sigs, &self.tarr, &self.arrs, y, k) {
                    self.fail(e);
                }
            }
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.tuple_expr(b, k);
            }
            Core::Call(g, args) => match &self.sigs[*g as usize] {
                Some(sig) if sig.ret == Kind::SK(k) => self.args(sig.params.clone(), args),
                _ => { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", e))); } self.ok = false },
            },
            _ => { if self.why.is_none() { self.why = Some(format!("{:.160}", format!("{:?}", e))); } self.ok = false },
        }
    }

    /// Tail position; returns the tail kind (S1 / SK(k)) with its array
    /// mask, or sets !ok. Owned arrays still alive at a tail are freed there
    /// by the emitter.
    fn tail(&mut self, e: &Core, fid: u32) -> (Kind, Vec<bool>) {
        if !self.ok {
            return (Kind::No, vec![]);
        }
        match e {
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.tail(b, fid)
            }
            Core::If(c, x, y) => {
                self.expr(c);
                let before = self.live.clone();
                let a = self.tail(x, fid);
                self.live = before;
                let b = self.tail(y, fid);
                if a == b || b.0 == Kind::No {
                    a
                } else if a.0 == Kind::No {
                    b
                } else {
                    self.fail(e);
                    (Kind::No, vec![])
                }
            }
            Core::Tuple(items) => {
                let mask: Vec<bool> = items.iter().map(|it| self.akind(it)).collect();
                let moved: Vec<(usize, u64)> =
                    items.iter().enumerate().filter_map(|(j, it)| self.any(it).map(|d| (j, d))).collect();
                for (j, d) in moved {
                    self.consume(&items[j], d);
                }
                (Kind::SK(items.len()), mask)
            }
            Core::Call(g, args) => {
                match &self.sigs[*g as usize] {
                    Some(sig) => {
                        let sig = sig.clone();
                        self.args(sig.params.clone(), args);
                        if *g == fid {
                            (Kind::No, vec![]) // own kind, resolved by the caller of tail()
                        } else {
                            (sig.ret, sig.ra.clone())
                        }
                    }
                    None => {
                        self.fail(e);
                        (Kind::No, vec![])
                    }
                }
            }
            other => {
                if self.akind(other) {
                    let d = self.aexpr(other);
                    self.settle(other, d);
                    (Kind::S1, vec![true])
                } else {
                    self.expr(other);
                    (Kind::S1, vec![false])
                }
            }
        }
    }
}

/// `e` calls `g` somewhere.
fn calls_fn(e: &Core, g: u32) -> bool {
    e.any(&mut |e| if matches!(e, Core::Call(h, _) if *h == g) { Some(true) } else { None })
}

/// `e` calls a function other than `g` that does not always inline.
fn calls_other_real(m: &CoreModule, e: &Core, g: u32) -> bool {
    e.any(&mut |e| match e {
        Core::Call(h, _) if *h != g && crate::inline_attr(&m.fns[*h as usize].body).is_empty() => Some(true),
        _ => None,
    })
}

/// The prelude helper of a binary32 primitive.
pub(crate) fn f32_fn(p: mithril_front::core::Prim) -> &'static str {
    use mithril_front::core::Prim::*;
    match p {
        F32Add => "f32_add",
        F32Sub => "f32_sub",
        F32Mul => "f32_mul",
        F32Div => "f32_div",
        F32Sqrt => "f32_sqrt",
        F32Lt => "f32_lt",
        F32FromU32 => "f32_from_u32",
        F32ToU32 => "f32_to_u32",
        _ => unreachable!(),
    }
}

/// A variable or constant.
fn is_atom(e: &Core) -> bool {
    matches!(e, Core::Num(_) | Core::Var(_))
}

/// An expression that can be evaluated as a value join: no calls and no
/// array writes or allocations (reads are fine).
fn join_arm(e: &Core) -> bool {
    use mithril_front::core::Prim;
    match e {
        Core::Num(_) | Core::Var(_) | Core::Proj(..) => true,
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => join_arm(a) && join_arm(b),
        Core::If(c, t, f) => join_arm(c) && join_arm(t) && join_arm(f),
        Core::Let(_, r, b) => join_arm(r) && join_arm(b),
        Core::Tuple(xs) => xs.iter().all(join_arm),
        Core::Prim(Prim::ArrGet | Prim::ArrLen, xs) => xs.iter().all(join_arm),
        _ => false,
    }
}

/// Component count of a tuple-valued expression (`None` for scalars or
/// mixed shapes): a tuple literal, an `if` whose arms agree, a let chain
/// ending in one, or a call returning SK(k).
fn tuple_kind(sigs: &[Option<Sig>], e: &Core) -> Option<usize> {
    match e {
        Core::Tuple(items) => Some(items.len()),
        Core::If(_, x, y) => {
            let (a, b) = (tuple_kind(sigs, x), tuple_kind(sigs, y));
            if a == b {
                a
            } else {
                None
            }
        }
        Core::Let(_, _, b) => tuple_kind(sigs, b),
        Core::Call(g, _) => match &sigs[*g as usize] {
            Some(Sig { ret: Kind::SK(k), .. }) => Some(*k),
            _ => None,
        },
        _ => None,
    }
}

/// Max constant-Proj index observed on `Var(p)` in `e`, or None if `p` is
/// ever used bare (=> must be a plain int).
fn proj_shape(e: &Core, p: u32, bare: &mut bool, max: &mut i64) {
    match e {
        Core::Var(i) if *i == p => *bare = true,
        Core::Proj(b, i) => match &**b {
            Core::Var(t) if *t == p => *max = (*max).max(*i as i64),
            other => proj_shape(other, p, bare, max),
        },
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
            proj_shape(a, p, bare, max);
            proj_shape(b, p, bare, max);
        }
        Core::If(c, x, y) => {
            proj_shape(c, p, bare, max);
            proj_shape(x, p, bare, max);
            proj_shape(y, p, bare, max);
        }
        Core::Let(_, r, b) => {
            proj_shape(r, p, bare, max);
            proj_shape(b, p, bare, max);
        }
        Core::Call(_, a) | Core::Ctor(_, a) | Core::Tuple(a) | Core::Reuse(_, _, a) | Core::Prim(_, a) => {
            for x in a {
                proj_shape(x, p, bare, max);
            }
        }
        Core::Match(s, arms) => {
            proj_shape(s, p, bare, max);
            for (_, _, b) in arms {
                proj_shape(b, p, bare, max);
            }
        }
        _ => {}
    }
}

thread_local! {
    /// Scalar signatures of the module being emitted, for direct calls
    /// from dive code (`native_sig`).
    pub(crate) static SIGS: std::cell::RefCell<Vec<Option<Sig>>> = const { std::cell::RefCell::new(Vec::new()) };
    /// `LEAF[g]`: native `g` is call-free and loop-free. It settles no fuel
    /// itself; its one unit is counted at the call site (a register
    /// increment in native callers), so fuel still measures work.
    pub(crate) static LEAF: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// `BRIDGE_LIVE[g]`: native `g` can be reached through its dive bridge
    /// (it is the entry, a fold, or has a non-native caller).
    pub(crate) static BRIDGE_LIVE: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// `SHIFTED[g]`: native `g` holds its ints pre-shifted (`x << 8`); see
    /// `choose_reps`.
    pub(crate) static SHIFTED: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    /// `CTX[g]`: native `g` takes the worker context (it touches arrays,
    /// itself or through a callee); the rest keep their argument registers.
    pub(crate) static CTX: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The context argument of a call to native `g` (`"ctx, "` or nothing).
pub(crate) fn ctx_arg(g: u32) -> &'static str {
    if CTX.with(|c| c.borrow().get(g as usize).copied().unwrap_or(true)) {
        "ctx, "
    } else {
        ""
    }
}

/// Which native functions need the context: array prims or array params
/// or results, or a call to one that does (fixpoint).
pub(crate) fn needs_ctx(m: &CoreModule, sigs: &[Option<Sig>]) -> Vec<bool> {
    fn prims(e: &Core, out: &mut bool, calls: &mut Vec<u32>) {
        e.walk(&mut |e| match e {
            Core::Prim(p, _) if !p.is_f32() => *out = true,
            Core::Call(g, _) => calls.push(*g),
            _ => {}
        });
    }
    let n = m.fns.len();
    let mut calls = vec![Vec::new(); n];
    let mut need: Vec<bool> = (0..n)
        .map(|f| {
            let Some(sig) = &sigs[f] else { return true };
            let mut p = false;
            prims(&m.fns[f].body, &mut p, &mut calls[f]);
            p || sig.params.iter().any(|t| matches!(t, PTy::A | PTy::B)) || sig.ra.iter().any(|a| *a)
        })
        .collect();
    loop {
        let mut changed = false;
        for f in 0..n {
            if !need[f] && calls[f].iter().any(|g| need[*g as usize]) {
                need[f] = true;
                changed = true;
            }
        }
        if !changed {
            return need;
        }
    }
}

thread_local! {
    /// Test override of `choose_reps` (see `EmitOpts`).
    pub(crate) static FORCE_REP: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn shifted(g: u32) -> bool {
    SHIFTED.with(|l| l.borrow().get(g as usize).copied().unwrap_or(false))
}

/// Integer representation of each native function, chosen by a static op
/// count over its body. *Plain* ints are canonical i56 values in an i64: an
/// op whose range is not proven re-wraps (two shifts), and masked 32-bit
/// arithmetic runs in u32 for free; array elements (stored pre-shifted)
/// cost a shift per access. *Shifted* ints are held as `x << 8`: i64
/// wrapping is i56 wrapping, so add/sub/compare/min chains need no wrap,
/// but a var-by-var multiply, a right shift, an array index and division
/// each cost an op. The cheaper side wins; calls convert at the boundary.
pub(crate) fn choose_reps(m: &CoreModule, sigs: &[Option<Sig>]) -> Vec<bool> {
    use crate::range::{feeds_mask, feeds_mask32, low32_closed};
    use mithril_front::ast::BinOp;
    use mithril_front::core::Prim;
    struct C<'a> {
        r: &'a crate::range::Ranges,
        plain: usize,
        shf: usize,
    }
    fn nonconst(e: &Core) -> bool {
        !matches!(e, Core::Num(_))
    }
    fn walk(c: &mut C, e: &Core, low: bool, low32: bool) {
        match e {
            Core::Op2(op, x, y) if low32 && low32_closed(op) => {
                if *op == BinOp::Mul && nonconst(x) && nonconst(y) {
                    c.shf += 1;
                }
                walk(c, x, true, true);
                walk(c, y, *op != BinOp::Shl, *op != BinOp::Shl);
            }
            Core::Op2(op, x, y) => {
                if c.r.wrap(op, x, y, low) {
                    c.plain += 2;
                }
                c.shf += match op {
                    BinOp::Mul if nonconst(x) && nonconst(y) => 1,
                    BinOp::Shr => 1 + nonconst(y) as usize,
                    BinOp::Shl => nonconst(y) as usize,
                    BinOp::Div | BinOp::FloorDiv | BinOp::Mod => 2,
                    _ => 0,
                };
                walk(c, x, feeds_mask(op, y), feeds_mask32(op, y));
                walk(c, y, false, false);
            }
            Core::Let(x, r, b) => {
                let (lo, lo32) = (c.r.masked(*x), c.r.masked32(*x));
                walk(c, r, lo, lo32);
                walk(c, b, low, low32);
            }
            Core::Prim(p, xs) if p.is_f32() => {
                c.shf += xs.len() + 1;
                xs.iter().for_each(|x| walk(c, x, false, false));
            }
            Core::Prim(p, xs) => {
                if matches!(p, Prim::ArrGet | Prim::ArrSet) {
                    c.plain += 1;
                    c.shf += nonconst(&xs[1]) as usize;
                }
                xs.iter().for_each(|x| walk(c, x, false, false));
            }
            Core::If(a, t, f) => {
                walk(c, a, false, false);
                walk(c, t, low, low32);
                walk(c, f, low, low32);
            }
            Core::Match(sc, arms) => {
                walk(c, sc, false, false);
                arms.iter().for_each(|(_, _, b)| walk(c, b, low, low32));
            }
            _ => e.kids().into_iter().for_each(|k| walk(c, k, false, false)),
        }
    }
    let n = m.fns.len();
    if std::env::var_os("MITHRIL_PLAIN_INTS").is_some() {
        return vec![false; n];
    }
    if let Some(r) = FORCE_REP.with(|f| f.get()) {
        return (0..n).map(|f| r && sigs[f].is_some()).collect();
    }
    // local op counts
    let local: Vec<(usize, usize)> = (0..n)
        .map(|f| {
            if sigs[f].is_none() {
                return (0, 0);
            }
            let r = crate::range::Ranges::of(&m.fns[f].body);
            let mut c = C { r: &r, plain: 0, shf: 0 };
            walk(&mut c, &m.fns[f].body, false, false);
            (c.plain, c.shf)
        })
        .collect();
    // call edges between native functions, weighted by the int values
    // crossing them (args and results): each converts when the two sides
    // differ
    fn calls(e: &Core, out: &mut Vec<u32>) {
        e.walk(&mut |e| {
            if let Core::Call(g, _) = e {
                out.push(*g);
            }
        });
    }
    let ints = |g: usize| -> usize {
        let s = sigs[g].as_ref().unwrap();
        let ps: usize = s.params.iter().map(|p| match p {
            PTy::I => 1,
            PTy::T(k) => *k,
            _ => 0,
        }).sum();
        ps + s.ra.iter().filter(|a| !**a).count()
    };
    let mut edges: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for f in 0..n {
        if sigs[f].is_none() {
            continue;
        }
        let mut cs = Vec::new();
        calls(&m.fns[f].body, &mut cs);
        for g in cs {
            let g = g as usize;
            if g != f && sigs[g].is_some() {
                let w = ints(g);
                edges[f].push((g, w));
                edges[g].push((f, w));
            }
        }
    }
    let mut rep = vec![false; n];
    for _ in 0..2 * n + 2 {
        let mut changed = false;
        for f in 0..n {
            if sigs[f].is_none() {
                continue;
            }
            let (p, q) = local[f];
            let cross = |sh: bool| -> usize { edges[f].iter().filter(|(g, _)| rep[*g] != sh).map(|(_, w)| w).sum() };
            let want = q + cross(true) < p + cross(false);
            if want != rep[f] {
                rep[f] = want;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    rep
}

/// Convert a native int expression between representations.
fn conv(e: E, from_sh: bool, to_sh: bool) -> E {
    match (from_sh, to_sh) {
        (false, true) => bin(Bop::Shl, e, i64_(8)),
        (true, false) => bin(Bop::Shr, e, i64_(8)),
        _ => e,
    }
}

pub(crate) fn is_leaf(g: u32) -> bool {
    LEAF.with(|l| l.borrow().get(g as usize).copied().unwrap_or(false))
}

/// Fuel accounting a native caller emits for a call to `g`.
fn leaf_unit(g: u32, b: &mut Vec<S>) {
    if is_leaf(g) {
        b.push(set("fl", bin(Bop::Add, v("fl"), i64_(1))));
    }
}

/// The signature of `g` when dive code may call its native form directly:
/// scalar-lowered, all-int parameters, and never suspending (a forking
/// scalar function keeps a dive form of its own and is not included).
pub(crate) fn native_sig(g: u32) -> Option<Sig> {
    SIGS.with(|s| s.borrow().get(g as usize).cloned().flatten())
        .filter(|s| s.params.iter().all(|p| *p == PTy::I) && s.ra.iter().all(|a| !a))
}

/// A tuple param's component count cannot be read off the body alone (a
/// caller may pass a wider tuple); seed with maxproj+1 and demote on caller
/// mismatch. Bare use of a param whose callers pass tuples also demotes.
pub(crate) fn classify(m: &CoreModule, tys: &crate::ty::Types) -> Vec<Option<Sig>> {
    use crate::ty::Ty;
    let n = m.fns.len();
    // a value native code cannot represent at the boundary (a non-int
    // array) rules the function out for good
    let forbid: Vec<bool> = (0..n)
        .map(|fi| {
            tys.params[fi].iter().chain(std::iter::once(&tys.ret[fi])).any(|t| matches!(t, Ty::Arr(false)))
        })
        .collect();
    let mut sigs: Vec<Option<Sig>> = m
        .fns
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            if forbid[fi] {
                return None;
            }
            let params = (0..f.arity as u32)
                .map(|p| {
                    if tys.params[fi].get(p as usize) == Some(&Ty::Arr(true)) {
                        return PTy::B; // optimistic: demoted when consumed
                    }
                    let (mut bare, mut mx) = (false, -1i64);
                    proj_shape(&f.body, p, &mut bare, &mut mx);
                    if !bare && mx >= 0 {
                        PTy::T((mx + 1) as usize)
                    } else {
                        PTy::I
                    }
                })
                .collect();
            // seed the return kind from type inference: mutually recursive
            // functions returning tuples cannot discover it from each other
            let (ret, ra) = match tys.ret.get(fi) {
                Some(Ty::Tup(k)) => (Kind::SK(*k as usize), vec![false; *k as usize]),
                Some(Ty::Arr(true)) => (Kind::S1, vec![true]),
                _ => (Kind::S1, vec![false]),
            };
            Some(Sig { params, ret, ra })
        })
        .collect();
    // Param types are body-derived (array params start borrowed and are
    // demoted to owned when a body consumes them); rets/eligibility are
    // recomputed for EVERY fn each round (a demotion is not sticky: a fn
    // rejected while its callee's return kind was still a guess re-qualifies
    // once the callee settles). Bounded rounds; convergence break.
    let mut param_seed: Vec<Vec<PTy>> =
        sigs.iter().enumerate().map(|(fi, s)| s.as_ref().map(|s| s.params.clone()).unwrap_or_else(|| vec![PTy::I; m.fns[fi].arity])).collect();
    fn run<'a>(m: &CoreModule, sigs: &'a [Option<Sig>], fid: usize, params: &[PTy]) -> (Chk<'a>, (Kind, Vec<bool>)) {
        let mut c = Chk {
            why: None,
            sigs,
            tvars: HashMap::new(),
            ok: true,
            arrs: HashMap::new(),
            tarr: HashMap::new(),
            live: Default::default(),
            demote: Vec::new(),
            root: HashMap::new(),
        };
        for (p, pt) in params.iter().enumerate() {
            match pt {
                PTy::T(k) => {
                    c.tvars.insert(p as u32, *k);
                }
                PTy::A => {
                    c.arrs.insert(p as u32, false);
                    c.live.insert(p as u64);
                }
                PTy::B => {
                    c.arrs.insert(p as u32, true);
                }
                PTy::I => {}
            }
        }
        let tk = c.tail(&m.fns[fid].body, fid as u32);
        (c, tk)
    }
    let seed = sigs.clone();
    let mut whys: Vec<Option<String>> = vec![None; n];
    for _ in 0..3 * n + 4 {
        let mut next = sigs.clone();
        let mut demoted = false;
        for fid in 0..n {
            if forbid[fid] {
                continue;
            }
            let params = param_seed[fid].clone();
            let guess = sigs[fid].as_ref().or(seed[fid].as_ref()).map(|s| (s.ret, s.ra.clone())).unwrap_or((Kind::S1, vec![false]));
            // a function is checked assuming its own signature (self calls)
            let mut own = sigs.clone();
            if own[fid].is_none() {
                own[fid] = Some(Sig { params: params.clone(), ret: guess.0, ra: guess.1.clone() });
            }
            let (c, tk) = run(m, &own, fid, &params);
            whys[fid] = c.why.clone();
            for v in &c.demote {
                if let Some(PTy::B) = param_seed[fid].get(*v as usize) {
                    param_seed[fid][*v as usize] = PTy::A;
                    demoted = true;
                }
            }
            next[fid] = if !c.ok {
                None
            } else {
                let (ret, ra) = match tk {
                    (Kind::No, _) => guess, // only self tail calls: keep guess
                    k => k,
                };
                Some(Sig { params, ret, ra })
            };
        }
        if next == sigs && !demoted {
            break;
        }
        sigs = next;
    }
    // final verification (stale optimistic ret guesses)
    let snapshot = sigs.clone();
    for fid in 0..n {
        let Some(sig) = &snapshot[fid] else { continue };
        let (c, tk) = run(m, &snapshot, fid, &sig.params);
        let consistent = c.ok && c.demote.is_empty() && (tk.0 == Kind::No || (tk.0 == sig.ret && tk.1 == sig.ra));
        if !consistent {
            sigs[fid] = None;
        }
    }
    if std::env::var_os("MITHRIL_DEBUG_SCALAR").is_some() {
        for (fid, f) in m.fns.iter().enumerate() {
            match &sigs[fid] {
                None => {
                    eprintln!("not scalar {fid} {}: {}", f.name, whys[fid].clone().unwrap_or_else(|| "ret kind".into()));
                }
                Some(sig) => eprintln!("scalar {fid} {}: {:?} -> {:?} {:?}", f.name, sig.params, sig.ret, sig.ra),
            }
        }
    }
    sigs
}

// ---------------------------------------------------------------- emission

struct Sem<'m> {
    sigs: &'m [Option<Sig>],
    ranges: crate::range::Ranges,
    /// the Op2 about to be emitted only has its low bits observed
    low: bool,
    /// ... only its low 32 bits
    low32: bool,
    tmp: u32,
    /// component vars (SK-destructured lets and T(k) params): var -> arity;
    /// components live as q<var>_<i>
    tvars: HashMap<u32, usize>,
    /// array vars -> borrowed; tuple vars -> array components; owned array
    /// slots alive on the current path (mirrors the classifier's `Chk`)
    arrs: HashMap<u32, bool>,
    tarr: HashMap<u32, Vec<bool>>,
    live: std::collections::BTreeSet<u64>,
    /// the function counts fuel (it loops or calls); a call-free, loop-free
    /// body does bounded work and settles none
    fuel: bool,
    /// this function's ints are pre-shifted (see `choose_reps`)
    shifted: bool,
    /// array value name -> the local holding its length (read once where
    /// the array enters; a write or a move keeps it), so bounds checks
    /// compare against a register instead of reloading the header
    lens: HashMap<String, String>,
    /// length locals are used in this function (see `scalar_fn`)
    use_lens: bool,
}

/// The name an array value is known by (arrays are always locals).
fn name_of(e: &E) -> String {
    crate::lir::rust::ex(e)
}

/// `x as u64`
fn as_u(e: E) -> E {
    cast(e, Ty::U64)
}

impl<'m> Sem<'m> {
    fn settle_fuel(&self, b: &mut Vec<S>) {
        if self.fuel {
            b.push(S::Store("fuel".into(), bin(Bop::Sub, E::Deref("fuel".into()), v("fl"))));
        }
    }

    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("s{}", self.tmp)
    }

    fn akind(&self, e: &Core) -> bool {
        akind(self.sigs, &self.tarr, &self.arrs, e)
    }

    fn slot(&self, e: &Core) -> Option<u64> {
        slot_key(e, &self.tarr, &self.arrs)
    }

    /// An int operand as a plain (unshifted) i64: a constant as is,
    /// anything else shifted down.
    fn unshifted(&mut self, e: &Core, b: &mut Vec<S>) -> E {
        match e {
            Core::Num(n) => i64_(*n),
            _ if !self.shifted => self.val(e, b),
            _ => bin(Bop::Shr, self.val(e, b), i64_(8)),
        }
    }

    /// An int value in the array storage form (pre-shifted).
    fn stored(&mut self, e: &Core, b: &mut Vec<S>) -> E {
        let x = self.val(e, b);
        conv(x, self.shifted, true)
    }

    /// The length local of array value `a`, reading it now if unknown
    /// (only where a loop carries it).
    fn len_of(&mut self, a: &str, b: &mut Vec<S>) -> String {
        if let Some(l) = self.lens.get(a) {
            return l.clone();
        }
        let l = format!("l_{a}");
        b.push(let_(&l, Ty::Usize, p("arr_len_of", vec![as_u(v(a))])));
        self.lens.insert(a.to_string(), l.clone());
        l
    }

    /// The length local of array value `a` when known.
    fn acc(&self, a: &str) -> Option<String> {
        self.lens.get(a).cloned()
    }

    /// Name of an array read in place (a live slot or a borrowed var).
    fn rd(&self, e: &Core) -> String {
        match e {
            Core::Var(x) => vn(*x),
            _ => slot_name(self.slot(e).expect("array read of a non-slot in scalar emission")),
        }
    }

    /// An array-valued expression in a consuming position; a slot operand
    /// is returned by name with its key for deferred consumption.
    fn aval(&mut self, e: &Core, b: &mut Vec<S>) -> (E, Option<u64>) {
        use mithril_front::core::Prim;
        if let Some(k) = self.slot(e) {
            return (v(slot_name(k)), Some(k));
        }
        let t = self.fresh();
        match e {
            Core::Prim(Prim::ArrNew, xs) => {
                let n = self.unshifted(&xs[0], b);
                let x = self.stored(&xs[1], b);
                b.push(let_(&t, Ty::I64, cast(p("arr_new_raw", vec![n, x]), Ty::I64)));
                if self.use_lens {
                    b.push(let_(format!("l_{t}"), Ty::Usize, p("arr_len_of", vec![as_u(v(&t))])));
                    self.lens.insert(t.clone(), format!("l_{t}"));
                }
            }
            Core::Prim(Prim::ArrSet, xs) => {
                let (a, d) = self.aval(&xs[0], b);
                let i = self.unshifted(&xs[1], b);
                let x = self.stored(&xs[2], b);
                if let Some(k) = d {
                    self.live.remove(&k);
                }
                let an = name_of(&a);
                match self.acc(&an) {
                    Some(l) => {
                        b.push(let_(&t, Ty::I64, cast(p("arr_set_n", vec![as_u(a), v(&l), i, as_u(x)]), Ty::I64)));
                        self.lens.insert(t.clone(), l);
                    }
                    None => b.push(let_(&t, Ty::I64, cast(p("arr_set_u", vec![as_u(a), i, as_u(x)]), Ty::I64))),
                }
            }
            Core::Call(g, args) => {
                let call = self.call(*g, args, b);
                b.push(let_(&t, Ty::I64, call));
            }
            Core::If(cd, x, y) => {
                let ec = self.val(cd, b);
                let before = self.live.clone();
                let lens0 = self.lens.clone();
                let mut bx = Vec::new();
                let vx = self.aval_now(x, &mut bx);
                bx.push(set(&t, vx));
                let after = std::mem::replace(&mut self.live, before);
                self.lens = lens0.clone();
                let mut by = Vec::new();
                let vy = self.aval_now(y, &mut by);
                by.push(set(&t, vy));
                self.live = after;
                self.lens = lens0;
                b.push(S::Decl(t.clone(), Ty::I64));
                b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bx, by));
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                return (self.aval_now(bo, b), None);
            }
            _ => unreachable!("non-array Core in scalar array emission"),
        }
        (v(t), None)
    }

    /// `aval`, consuming a slot operand immediately.
    fn aval_now(&mut self, e: &Core, b: &mut Vec<S>) -> E {
        let (x, d) = self.aval(e, b);
        if let Some(k) = d {
            self.live.remove(&k);
        }
        x
    }

    /// Any value in a consuming position (int or array).
    fn anyval(&mut self, e: &Core, b: &mut Vec<S>) -> (E, Option<u64>) {
        if self.akind(e) {
            self.aval(e, b)
        } else {
            (self.val(e, b), None)
        }
    }

    /// The call `s_g(ctx?, fuel, args..)` with its fuel accounting.
    fn raw_call(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> E {
        let es = self.call_args(g, args, b);
        leaf_unit(g, b);
        let mut a = vec![v("fuel")];
        a.extend(es);
        E::Call { f: format!("s_{g}"), ctx: !ctx_arg(g).is_empty(), args: a }
    }

    /// A single-value call's result in this function's representation.
    fn call(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> E {
        let call = self.raw_call(g, args, b);
        let sig = self.sigs[g as usize].as_ref().unwrap();
        if shifted(g) == self.shifted || sig.ra.first() == Some(&true) {
            call
        } else {
            conv(call, shifted(g), self.shifted)
        }
    }

    /// A tuple-valued call: its components, converted to this function's
    /// representation (array components pass as they are).
    fn call_tuple(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> Vec<E> {
        let call = self.raw_call(g, args, b);
        let sig = self.sigs[g as usize].as_ref().unwrap().clone();
        let Kind::SK(k) = sig.ret else { unreachable!() };
        let t = self.fresh();
        let rs: Vec<String> = (0..k).map(|i| format!("{t}_{i}")).collect();
        b.push(S::Let(Pat::Tup(rs.clone()), Ty::Infer, call));
        (0..k).map(|i| if sig.ra[i] || shifted(g) == self.shifted { v(&rs[i]) } else { conv(v(&rs[i]), shifted(g), self.shifted) }).collect()
    }

    /// Free the owned arrays still alive at a path's end.
    fn drop_live(&self, b: &mut Vec<S>) {
        for k in &self.live {
            b.push(free(as_u(v(slot_name(*k)))));
        }
    }

    /// The mutable local names of `fid`'s flattened parameter list, in
    /// flattened order (v<p> for ints, q<p>_<i> for tuple components).
    fn slot_names(&self, fid: u32) -> Vec<String> {
        let sig = self.sigs[fid as usize].as_ref().unwrap();
        let mut out = Vec::new();
        for (pp, pt) in sig.params.iter().enumerate() {
            match pt {
                PTy::I | PTy::A | PTy::B => out.push(vn(pp as u32)),
                PTy::T(k) => {
                    for i in 0..*k {
                        out.push(format!("q{pp}_{i}"));
                    }
                }
            }
        }
        out
    }

    /// Flatten call args per the callee's parameter types (tuple params
    /// expand to k component expressions).
    fn call_args(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> Vec<E> {
        let sig = self.sigs[g as usize].clone().expect("call to non-scalar in scalar emission");
        let mut es = Vec::new();
        let mut moved = Vec::new();
        for (pt, a) in sig.params.iter().zip(args) {
            match pt {
                PTy::A => {
                    let (x, d) = self.aval(a, b);
                    moved.extend(d);
                    es.push(x);
                }
                PTy::B => es.push(v(self.rd(a))),
                PTy::I => {
                    let x = self.val(a, b);
                    es.push(conv(x, self.shifted, shifted(g)));
                }
                PTy::T(k) => match a {
                    Core::Var(t) if self.tvars.contains_key(t) => {
                        for i in 0..*k {
                            es.push(conv(v(format!("q{t}_{i}")), self.shifted, shifted(g)));
                        }
                    }
                    Core::Tuple(items) => {
                        for it in items {
                            let x = self.val(it, b);
                            es.push(conv(x, self.shifted, shifted(g)));
                        }
                    }
                    _ => unreachable!("non-idiom tuple arg in scalar emission"),
                },
            }
        }
        for k in moved {
            self.live.remove(&k);
        }
        es
    }

    fn bind(&mut self, x: u32, r: &Core, b: &mut Vec<S>) {
        if let Some(k) = tuple_kind(self.sigs, r) {
            let mask = tmask(self.sigs, &self.tarr, &self.arrs, r, k);
            let comps = self.tval(r, b);
            for (i, e) in comps.into_iter().enumerate() {
                b.push(let_(format!("q{x}_{i}"), Ty::I64, e));
            }
            self.tvars.insert(x, k);
            for (i, a) in mask.iter().enumerate() {
                if *a {
                    self.live.insert((1u64 << 40) | ((x as u64) << 8) | i as u64);
                }
            }
            self.tarr.insert(x, mask);
            return;
        }
        if self.akind(r) {
            if let Core::Var(y) = r {
                if self.arrs.get(y) == Some(&true) {
                    self.arrs.insert(x, true);
                    b.push(let_(vn(x), Ty::I64, v(vn(*y))));
                    if let Some(l) = self.lens.get(&vn(*y)).cloned() {
                        self.lens.insert(vn(x), l);
                    }
                    return;
                }
            }
            let er = self.aval_now(r, b);
            let en = name_of(&er);
            b.push(let_(vn(x), Ty::I64, er));
            if let Some(l) = self.lens.get(&en).cloned() {
                self.lens.insert(vn(x), l);
            }
            self.arrs.insert(x, false);
            self.live.insert(x as u64);
            return;
        }
        self.low = self.ranges.masked(x);
        self.low32 = self.ranges.masked32(x);
        let er = self.val(r, b);
        b.push(let_(vn(x), Ty::I64, er));
    }

    /// A tuple-valued expression as its native components.
    fn tval(&mut self, e: &Core, b: &mut Vec<S>) -> Vec<E> {
        match e {
            Core::Tuple(items) => {
                let mut moved = Vec::new();
                let es: Vec<E> = items
                    .iter()
                    .map(|it| {
                        let (x, d) = self.anyval(it, b);
                        moved.extend(d);
                        x
                    })
                    .collect();
                for k in moved {
                    self.live.remove(&k);
                }
                es
            }
            Core::If(cd, x, y) => {
                let k = tuple_kind(self.sigs, e).expect("tuple if");
                let ec = self.val(cd, b);
                let t = self.fresh();
                let names: Vec<String> = (0..k).map(|i| format!("{t}_{i}")).collect();
                let before = self.live.clone();
                let lens0 = self.lens.clone();
                let mut bx = Vec::new();
                for (n, x) in names.iter().zip(self.tval(x, &mut bx)) {
                    bx.push(set(n, x));
                }
                let after = std::mem::replace(&mut self.live, before);
                self.lens = lens0.clone();
                let mut by = Vec::new();
                for (n, y) in names.iter().zip(self.tval(y, &mut by)) {
                    by.push(set(n, y));
                }
                self.live = after;
                self.lens = lens0;
                for n in &names {
                    b.push(S::Decl(n.clone(), Ty::I64));
                }
                b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bx, by));
                names.into_iter().map(v).collect()
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.tval(bo, b)
            }
            Core::Call(g, args) => self.call_tuple(*g, args, b),
            _ => unreachable!("non-tuple Core in scalar tuple emission"),
        }
    }

    fn val(&mut self, e: &Core, b: &mut Vec<S>) -> E {
        let low = std::mem::take(&mut self.low);
        let low32 = std::mem::take(&mut self.low32);
        if !self.shifted {
            match e {
                Core::Num(n) => return i64_(*n),
                Core::Prim(mithril_front::core::Prim::ArrGet, xs) => {
                    let a = self.rd(&xs[0]);
                    let i = self.val(&xs[1], b);
                    let t = self.fresh();
                    let get = match self.acc(&a) {
                        Some(l) => p("arr_get_n", vec![as_u(v(&a)), v(l), i]),
                        None => p("arr_get_r", vec![as_u(v(&a)), i]),
                    };
                    b.push(let_(&t, Ty::I64, bin(Bop::Shr, cast(get, Ty::I64), i64_(8))));
                    return v(t);
                }
                Core::Prim(mithril_front::core::Prim::ArrLen, xs) => {
                    let a = self.rd(&xs[0]);
                    return match self.acc(&a) {
                        Some(l) => cast(v(l), Ty::I64),
                        None => cast(p("arr_len_of", vec![as_u(v(a))]), Ty::I64),
                    };
                }
                Core::Op2(..) | Core::Cmp(..) => return self.plain_arith(e, low, low32, b),
                Core::Prim(pr, xs) if pr.is_f32() => {
                    let es: Vec<E> = xs.iter().map(|x| self.val(x, b)).collect();
                    let t = self.fresh();
                    b.push(let_(&t, Ty::I64, p(f32_fn(*pr), es)));
                    return v(t);
                }
                _ => {}
            }
        }
        match e {
            // native ints are pre-shifted (`x << 8`): i64 wrapping is i56
            // wrapping, so no op needs a wrap fix-up
            Core::Num(n) => i64_((*n).wrapping_shl(8)),
            Core::Prim(mithril_front::core::Prim::ArrGet, xs) => {
                let a = self.rd(&xs[0]);
                let i = self.unshifted(&xs[1], b);
                let t = self.fresh();
                let get = match self.acc(&a) {
                    Some(l) => p("arr_get_n", vec![as_u(v(&a)), v(l), i]),
                    None => p("arr_get_r", vec![as_u(v(&a)), i]),
                };
                b.push(let_(&t, Ty::I64, cast(get, Ty::I64)));
                v(t)
            }
            Core::Prim(pr, xs) if pr.is_f32() => {
                // shifted representation: the bit patterns go through plain
                let es: Vec<E> = xs.iter().map(|x| self.unshifted(x, b)).collect();
                let t = self.fresh();
                b.push(let_(&t, Ty::I64, bin(Bop::Shl, p(f32_fn(*pr), es), i64_(8))));
                v(t)
            }
            Core::Prim(mithril_front::core::Prim::ArrLen, xs) => {
                let a = self.rd(&xs[0]);
                match self.acc(&a) {
                    Some(l) => bin(Bop::Shl, cast(v(l), Ty::I64), i64_(8)),
                    None => bin(Bop::Shl, cast(p("arr_len_of", vec![as_u(v(a))]), Ty::I64), i64_(8)),
                }
            }
            Core::Var(i) => v(vn(*i)),
            Core::Proj(base, i) => match &**base {
                Core::Var(t) if self.tvars.contains_key(t) => v(format!("q{t}_{i}")),
                _ => unreachable!("non-idiom Proj in scalar emission"),
            },
            Core::Op2(op, x, y) => {
                let t = self.fresh();
                let k = |e: &Core| if let Core::Num(n) = e { Some(*n) } else { None };
                let body = match bin_code(op) {
                    c @ (0 | 1 | 8 | 9 | 10) => {
                        let (ex, ey) = (self.val(x, b), self.val(y, b));
                        arith(c, ex, ey)
                    }
                    2 => match (k(x), k(y)) {
                        // one factor unshifted: a constant as is
                        (_, Some(cst)) => bin(Bop::Mul, self.val(x, b), i64_(cst)),
                        (Some(cst), _) => bin(Bop::Mul, self.val(y, b), i64_(cst)),
                        _ => {
                            let (ex, ey) = (self.val(x, b), self.val(y, b));
                            bin(Bop::Mul, ex, bin(Bop::Shr, ey, i64_(8)))
                        }
                    },
                    6 | 7 => {
                        let ex = self.val(x, b);
                        let sh = self.unshifted(y, b);
                        if bin_code(op) == 6 {
                            bin(Bop::Shl, ex, sh)
                        } else {
                            bin(Bop::And, bin(Bop::Shr, ex, sh), i64_(-256))
                        }
                    }
                    c => {
                        // division family: on unshifted values
                        let ex = self.unshifted(x, b);
                        let ey = self.unshifted(y, b);
                        bin(Bop::Shl, arith(c, ex, ey), i64_(8))
                    }
                };
                b.push(let_(&t, Ty::I64, body));
                v(t)
            }
            Core::Cmp(op, x, y) => {
                let (ex, ey) = (self.val(x, b), self.val(y, b));
                bin(Bop::Shl, cast(compare(cmp_code(op), ex, ey), Ty::I64), i64_(8))
            }
            Core::If(cd, x, y) if is_atom(x) && is_atom(y) => {
                // a select of computed values (what if-conversion leaves):
                // mask arithmetic, so the backend cannot turn it back into
                // an unpredictable branch on a loop-carried chain
                let ec = self.val(cd, b);
                let (vx, vy) = (self.val(x, b), self.val(y, b));
                let t = self.fresh();
                let m = format!("{t}m");
                b.push(let_(&m, Ty::I64, E::Neg(Box::new(cast(bin(Bop::Ne, ec, i64_(0)), Ty::I64)))));
                b.push(let_(&t, Ty::I64, bin(Bop::Or, bin(Bop::And, vx, v(&m)), bin(Bop::And, vy, bin(Bop::Xor, v(&m), i64_(-1))))));
                v(t)
            }
            Core::If(cd, x, y) => {
                let ec = self.val(cd, b);
                let t = self.fresh();
                let before = self.live.clone();
                let lens0 = self.lens.clone();
                let mut bx = Vec::new();
                let vx = self.val(x, &mut bx);
                bx.push(set(&t, vx));
                let after = std::mem::replace(&mut self.live, before);
                self.lens = lens0.clone();
                let mut by = Vec::new();
                let vy = self.val(y, &mut by);
                by.push(set(&t, vy));
                self.live = after;
                self.lens = lens0;
                b.push(S::Decl(t.clone(), Ty::I64));
                b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bx, by));
                v(t)
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.val(bo, b)
            }
            Core::Call(g, args) => {
                let call = self.call(*g, args, b);
                let t = self.fresh();
                b.push(let_(&t, Ty::I64, call));
                v(t)
            }
            _ => unreachable!("non-scalar Core in scalar emission"),
        }
    }

    /// Plain-representation arithmetic: canonical i56 values, re-wrapped
    /// where the range is not proven; masked 32-bit ops run in u32.
    fn plain_arith(&mut self, e: &Core, low: bool, low32: bool, b: &mut Vec<S>) -> E {
        match e {
            Core::Op2(op, x, y) if low32 && crate::range::low32_closed(op) => {
                // only the low 32 bits are observed: compute in u32 (the
                // port's `& 0xFFFFFFFF` masks become free)
                self.low32 = true;
                self.low = true;
                let ex = self.val(x, b);
                self.low32 = *op != mithril_front::ast::BinOp::Shl;
                self.low = *op != mithril_front::ast::BinOp::Shl;
                let ey = self.val(y, b);
                let t = self.fresh();
                let body = arith(bin_code(op), cast(ex, Ty::U32), cast(ey, Ty::U32));
                b.push(let_(&t, Ty::I64, cast(body, Ty::I64)));
                v(t)
            }
            Core::Op2(op, x, y) => {
                self.low = crate::range::feeds_mask(op, y);
                self.low32 = crate::range::feeds_mask32(op, y);
                let ex = self.val(x, b);
                let ey = self.val(y, b);
                let t = self.fresh();
                let wrap = self.ranges.wrap(op, x, y, low);
                let body = arith(bin_code(op), ex, ey);
                b.push(let_(&t, Ty::I64, if wrap { p("wrap56", vec![body]) } else { body }));
                v(t)
            }
            Core::Cmp(op, x, y) => {
                let (ex, ey) = (self.val(x, b), self.val(y, b));
                cast(compare(cmp_code(op), ex, ey), Ty::I64)
            }
            _ => unreachable!(),
        }
    }

    /// Tail position: self tail calls become loop iterations; everything
    /// else returns (a bare i64 for S1, a native tuple for SK).
    fn tail(&mut self, e: &Core, fid: u32, lp: bool, b: &mut Vec<S>) {
        match e {
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.tail(bo, fid, lp, b);
            }
            Core::If(..) if join_arm(e) && !self.akind(e) && tuple_kind(self.sigs, e).is_none_or(|k| {
                tmask(self.sigs, &self.tarr, &self.arrs, e, k).iter().all(|a| !a)
            }) =>
            {
                // call-free, array-free arms: one value join and a single
                // return (per-arm returns each settle fuel, which keeps the
                // backend from turning a cheap unpredictable branch into a
                // select)
                let x = match tuple_kind(self.sigs, e) {
                    Some(_) => E::Tup(self.tval(e, b)),
                    None => self.val(e, b),
                };
                self.drop_live(b);
                self.settle_fuel(b);
                b.push(ret(x));
            }
            Core::If(cd, x, y) => {
                let ec = self.val(cd, b);
                let (live, arrs, tarr, lens) = (self.live.clone(), self.arrs.clone(), self.tarr.clone(), self.lens.clone());
                let mut bx = Vec::new();
                self.tail(x, fid, lp, &mut bx);
                (self.live, self.arrs, self.tarr, self.lens) = (live, arrs, tarr, lens.clone());
                let mut by = Vec::new();
                self.tail(y, fid, lp, &mut by);
                self.lens = lens;
                b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bx, by));
            }
            Core::Call(g, args) if *g == fid && lp => {
                let es = self.call_args(*g, args, b);
                for (i, ea) in es.iter().enumerate() {
                    b.push(let_(format!("n{i}"), Ty::I64, ea.clone()));
                }
                self.drop_live(b);
                let sig = self.sigs[fid as usize].clone().unwrap();
                let mut nl = Vec::new();
                for (pp, pt) in sig.params.iter().enumerate() {
                    if self.use_lens && matches!(pt, PTy::A | PTy::B) {
                        // es is flattened; array params occupy one slot each
                        let j = self.slot_names(fid).iter().position(|n| *n == vn(pp as u32)).unwrap();
                        let l = self.len_of(&name_of(&es[j]), b);
                        nl.push((pp, l));
                    }
                }
                for (i, slot) in self.slot_names(fid).into_iter().enumerate() {
                    b.push(set(slot, v(format!("n{i}"))));
                }
                for (pp, l) in nl {
                    b.push(set(format!("l_v{pp}"), v(l)));
                }
                b.push(S::Continue);
            }
            Core::Call(g, args) => {
                let call = match self.sigs[*g as usize].as_ref().map(|s| s.ret) {
                    Some(Kind::SK(_)) => {
                        let comps = self.call_tuple(*g, args, b);
                        E::Tup(comps)
                    }
                    _ => self.call(*g, args, b),
                };
                self.drop_live(b);
                self.settle_fuel(b);
                b.push(ret(call));
            }
            Core::Tuple(_) => {
                let x = E::Tup(self.tval(e, b));
                self.drop_live(b);
                self.settle_fuel(b);
                b.push(ret(x));
            }
            other => {
                let x = if self.akind(other) { self.aval_now(other, b) } else { self.val(other, b) };
                self.drop_live(b);
                self.settle_fuel(b);
                b.push(ret(x));
            }
        }
    }
}

/// The native form `s_<fid>` plus the bridging dive form `d_<fid>`.
/// `bor[fid][p]` = param p is borrowed (bridge must not free a tuple arg's
/// spine; owned tuple args are freed after unpacking — components are NUMs,
/// so only the spine cells matter).
pub(crate) fn scalar_fn(m: &CoreModule, fid: u32, sigs: &[Option<Sig>], bor: &[Vec<bool>], bridge: bool) -> Vec<FnDef> {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    let lp = f.self_tail_rec;
    let sig = sigs[fid as usize].as_ref().unwrap().clone();
    let mut params: Vec<(String, Ty)> = vec![("fuel".into(), Ty::RefI64)];
    for (pp, pt) in sig.params.iter().enumerate() {
        match pt {
            PTy::I | PTy::A | PTy::B => params.push((vn(pp as u32), Ty::I64)),
            PTy::T(k) => {
                for i in 0..*k {
                    params.push((format!("q{pp}_{i}"), Ty::I64));
                }
            }
        }
    }
    let ret_ty = match sig.ret {
        Kind::S1 => Ty::I64,
        Kind::SK(k) => Ty::Tup(k),
        Kind::No => unreachable!(),
    };
    let mut sem = Sem {
        sigs,
        ranges: crate::range::Ranges::of(&f.body),
        low: false,
        low32: false,
        tmp: 0,
        tvars: HashMap::new(),
        arrs: HashMap::new(),
        tarr: HashMap::new(),
        live: Default::default(),
        fuel: !is_leaf(fid),
        shifted: shifted(fid),
        lens: HashMap::new(),
        use_lens: false,
    };
    for (pp, pt) in sig.params.iter().enumerate() {
        match pt {
            PTy::T(k) => {
                sem.tvars.insert(pp as u32, *k);
            }
            PTy::A => {
                sem.arrs.insert(pp as u32, false);
                sem.live.insert(pp as u64);
            }
            PTy::B => {
                sem.arrs.insert(pp as u32, true);
            }
            PTy::I => {}
        }
    }
    // loop-carried array params keep their length in a loop variable;
    // elsewhere a length is read at its first use
    // Length locals pay where the backend cannot keep a length itself: a
    // loop that writes an array and also accesses another one (the write
    // may alias the other's header, forcing a reload per access). With one
    // array, or only reads, the header load is hoisted already, and a
    // register length only obstructs the backend (e.g. vectorization).
    let arr_params = sig.params.iter().filter(|t| matches!(t, PTy::A | PTy::B)).count();
    let writes = sig.params.iter().any(|t| matches!(t, PTy::A));
    // Only in a loop whose body calls nothing but itself: the lengths
    // take registers, which a call-free loop body has to spare; a loop
    // with inlined callees is register-bound and reloading is cheaper.
    // (a call to a small call-free function that always inlines does not
    // count: after inlining the body is still call-free)
    let leaf_loop = calls_fn(&f.body, fid) && !calls_other_real(m, &f.body, fid);
    let looping = lp && leaf_loop && arr_params >= 2 && writes;
    sem.use_lens = looping;
    if looping {
        for (pp, pt) in sig.params.iter().enumerate() {
            if matches!(pt, PTy::A | PTy::B) {
                sem.lens.insert(vn(pp as u32), format!("l_v{pp}"));
            }
        }
    }
    let mut bb = Vec::new();
    sem.tail(&f.body, fid, lp, &mut bb);
    // one fuel unit per call and loop iteration, as in the dive form: native
    // code never suspends, but its work counts toward the enclosing dive's
    // budget (so parallel granularity tracks work, not dive calls). Counted
    // in a local and settled at each return, so it stays in a register.
    let mut body: Vec<S> = sig
        .params
        .iter()
        .enumerate()
        .filter(|(_, t)| looping && matches!(t, PTy::A | PTy::B))
        .map(|(pp, _)| let_(format!("l_v{pp}"), Ty::Usize, p("arr_len_of", vec![as_u(v(vn(pp as u32)))])))
        .collect();
    body.push(let_("fl", Ty::I64, i64_(1)));
    if lp {
        let mut lb = vec![set("fl", bin(Bop::Add, v("fl"), i64_(1)))];
        lb.extend(bb);
        body.push(S::Loop(lb));
    } else {
        body.extend(bb);
    }
    let inl = crate::seq::inline_of(crate::inline_attr_fn(m, fid));
    let native = FnDef { name: format!("s_{fid}"), ctx: !ctx_arg(fid).is_empty(), params, ret: ret_ty, body, inline: inl, cold: false };
    if !bridge {
        return vec![native];
    }

    // bridge: unpack ports per param type, call, repack per return kind
    let mut unpack = Vec::new();
    let mut bargs: Vec<E> = vec![v("fuel")];
    let mut after = Vec::new();
    let conv = |e: E| if shifted(fid) { p("sh", vec![e]) } else { as_i(e) };
    for (pp, pt) in sig.params.iter().enumerate() {
        let pv = v(vn(pp as u32));
        match pt {
            PTy::I => bargs.push(conv(pv)),
            // array ports pass as their bits; the dive side's borrow mode
            // for this param may differ from the native one
            PTy::A => {
                if bor[fid as usize][pp] {
                    unpack.push(let_(vn(pp as u32), Ty::U64, c("dup_val", vec![pv.clone()])));
                }
                // native code writes owned arrays without refcount checks
                unpack.push(let_(vn(pp as u32), Ty::U64, c("arr_own", vec![pv.clone()])));
                bargs.push(cast(pv, Ty::I64));
            }
            PTy::B => {
                if !bor[fid as usize][pp] {
                    after.push(free(pv.clone()));
                }
                bargs.push(cast(pv, Ty::I64));
            }
            PTy::T(k) => {
                for i in 0..*k {
                    let an = format!("a{pp}_{i}");
                    unpack.push(let_(&an, Ty::I64, conv(c("field", vec![pv.clone(), usize_(i)]))));
                    bargs.push(v(an));
                }
                if !bor[fid as usize][pp] {
                    unpack.push(free(pv));
                }
            }
        }
    }
    let pack = |i: usize, r: E| if sig.ra.get(i).copied().unwrap_or(false) { as_u(r) } else if shifted(fid) { p("retag", vec![r]) } else { num(r) };
    let call = E::Call { f: format!("s_{fid}"), ctx: !ctx_arg(fid).is_empty(), args: bargs };
    let mut bridge_body = unpack;
    if is_leaf(fid) {
        bridge_body.push(burn_fuel());
    }
    match sig.ret {
        Kind::S1 => {
            bridge_body.push(let_("r", Ty::I64, call));
            bridge_body.extend(after);
            bridge_body.push(ret(ok(pack(0, v("r")))));
        }
        Kind::SK(k) => {
            let comps: Vec<String> = (0..k).map(|i| format!("r{i}")).collect();
            bridge_body.push(S::Let(Pat::Tup(comps.clone()), Ty::Infer, call));
            bridge_body.extend(after);
            let packs: Vec<E> = (0..k).map(|i| pack(i, v(&comps[i]))).collect();
            bridge_body.push(ret(ok(c("mk_con", vec![u16_(0xFFF), E::Slice(packs)]))));
        }
        Kind::No => unreachable!(),
    }
    // Every caller native: nothing dives this function, so the bridge
    // has no caller and must not look like one (a live bridge call site
    // with unknown arguments blocks the backend's interprocedural
    // constant propagation and single-call-site inlining).
    if !BRIDGE_LIVE.with(|b| b.borrow().get(fid as usize).copied().unwrap_or(true)) {
        bridge_body = vec![S::Unreachable];
    }
    let mut dparams = vec![("fuel".to_string(), Ty::RefI64)];
    dparams.extend(vparams(ar));
    vec![native, FnDef { name: format!("d_{fid}"), ctx: true, params: dparams, ret: Ty::Res, body: bridge_body, inline: Inline::Default, cold: false }]
}
