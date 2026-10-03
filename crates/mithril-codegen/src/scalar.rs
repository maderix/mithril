//! Native scalar lowering.
//!
//! A function is *scalar* when every value it computes is an integer:
//! `Num / Var / Op2 / Cmp / If / Let`, calls to other scalar functions, and —
//! because `while`/`for` desugar into helpers that tail-return a `Tuple` of
//! the live loop variables — an all-integer `Tuple` in tail position
//! (kind `SK(k)`), consumed at call sites exclusively through the
//! `Let(t, Call(g), .. Proj(Var t, i) ..)` idiom the desugarer emits.
//! No floats, constructors, matches, or escaping tuples.
//!
//! Scalar functions are emitted as plain `fn s_<fid>(v0: i64, ..) -> i64`
//! (or `-> (i64, .., i64)` for `SK(k)`) with native 64-bit wrapping
//! arithmetic (exactly `bin`'s int path),
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

use crate::lir::{do_, bin, burn_fuel, c, cast, free, i64_, let_, ok, p, ret, set, u16_, usize_, v, Bop, FnDef, Inline, Pat, Ty, E, S};
use crate::seq::{arith, bin_code, cmp_code, compare, vn, vparams};
use mithril_front::core::{Core, CoreModule, Prim, UNREACHABLE_CTOR};
use crate::ty::Shape;
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
/// the end of each path) or borrows (`B`: only read; the caller keeps it),
/// or a constructor value it only matches on (`H`: borrowed, like `B`) or
/// consumes (`O`: its one use matches it, freeing the cells, or moves it on).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum PTy {
    I,
    T(usize),
    A,
    B,
    H,
    O,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Sig {
    pub params: Vec<PTy>,
    pub ret: Kind, // S1 or SK(k); No never appears inside a Some(Sig)
    /// which return components are (owned) int arrays: one entry for S1,
    /// k for SK(k)
    pub ra: Vec<bool>,
}

fn slot_name(key: u64) -> String {
    if key >> 40 != 0 {
        format!("q{}_{}", (key >> 8) & 0xFFFF_FFFF, key & 0xFF)
    } else {
        format!("v{key}")
    }
}

/// What a let binds (see `Chk::bind`).
enum Bound {
    Tup(Shape, Vec<bool>),
    /// an alias of a borrowed array: the RHS variable
    Lent(u32),
    Owned,
    Scalar,
}

/// Value bindings shared by classification and emission. Tuple variables
/// hold their leaves as q<var>_<i>; array variables record borrowing.
#[derive(Clone, Default)]
struct Scope {
    tvars: HashMap<u32, Shape>,
    arrs: HashMap<u32, bool>,
    tarr: HashMap<u32, Vec<bool>>,
}

impl Scope {
    /// An owned array slot: a variable, or component `i` of a destructured
    /// tuple variable `t`.
    fn slot(&self, e: &Core) -> Option<u64> {
        match e {
            Core::Var(v) if self.arrs.contains_key(v) => Some(*v as u64),
            Core::Proj(b, i) => match &**b {
                Core::Var(t) if self.tarr.get(t).is_some_and(|m| m.get(*i).copied().unwrap_or(false)) => {
                    Some((1u64 << 40) | ((*t as u64) << 8) | *i as u64)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether `e` is array-valued (under the current bindings).
    fn array(&self, sigs: &[Option<Sig>], e: &Core) -> bool {
        match e {
            Core::Var(v) => self.arrs.contains_key(v),
            Core::Proj(..) => self.slot(e).is_some(),
            Core::Prim(Prim::ArrNew | Prim::ArrSet, _) => true,
            Core::Call(g, _) => matches!(&sigs[*g as usize], Some(s) if s.ret == Kind::S1 && s.ra.first() == Some(&true)),
            Core::If(_, x, _) => self.array(sigs, x),
            Core::Let(_, _, b) => self.array(sigs, b),
            _ => false,
        }
    }

    /// Array mask of a tuple-valued expression of `k` components.
    fn mask(&self, sigs: &[Option<Sig>], e: &Core, k: usize) -> Vec<bool> {
        match e {
            Core::Tuple(items) if items.len() == k => items.iter().map(|it| self.array(sigs, it)).collect(),
            // a nested tuple holds no arrays (the checker enforces it)
            Core::Tuple(_) => vec![false; k],
            Core::If(_, x, _) => self.mask(sigs, x, k),
            Core::Let(_, _, b) => self.mask(sigs, b, k),
            Core::Call(g, _) => sigs[*g as usize].as_ref().map(|s| s.ra.clone()).unwrap_or_else(|| vec![false; k]),
            _ => vec![false; k],
        }
    }

    fn bound(&self, sigs: &[Option<Sig>], r: &Core) -> Bound {
        if let Some(sh) = tuple_kind(sigs, &self.tvars, r) {
            let mask = self.mask(sigs, r, sh.width());
            Bound::Tup(sh, mask)
        } else if self.array(sigs, r) {
            match r {
                Core::Var(v) if self.arrs.get(v) == Some(&true) => Bound::Lent(*v),
                _ => Bound::Owned,
            }
        } else {
            Bound::Scalar
        }
    }

    /// Install after the RHS is checked/emitted and its local scope ends.
    fn bind(&mut self, x: u32, bound: Bound, live: &mut std::collections::BTreeSet<u64>) {
        self.tvars.remove(&x);
        self.tarr.remove(&x);
        self.arrs.remove(&x);
        match bound {
            Bound::Tup(sh, mask) => {
                for (i, a) in mask.iter().enumerate() {
                    if *a {
                        live.insert((1u64 << 40) | ((x as u64) << 8) | i as u64);
                    }
                }
                self.tvars.insert(x, sh);
                self.tarr.insert(x, mask);
            }
            Bound::Lent(_) | Bound::Owned => {
                let borrowed = matches!(bound, Bound::Lent(_));
                self.arrs.insert(x, borrowed);
                if !borrowed {
                    live.insert(x as u64);
                }
            }
            Bound::Scalar => {}
        }
    }
}

/// The same native parameter bindings seed both the checker and emitter.
fn param_bindings(fid: u32, params: &[PTy]) -> (Scope, std::collections::BTreeSet<u64>, HashMap<u32, bool>) {
    let (mut scope, mut live, mut handles) = (Scope::default(), std::collections::BTreeSet::new(), HashMap::new());
    for (p, pt) in params.iter().enumerate() {
        match pt {
            PTy::T(k) => {
                scope.tvars.insert(p as u32, pshape(fid, p, *k));
            }
            PTy::A | PTy::B => scope.bind(p as u32, if *pt == PTy::B { Bound::Lent(p as u32) } else { Bound::Owned }, &mut live),
            PTy::H | PTy::O => {
                handles.insert(p as u32, *pt == PTy::O);
            }
            PTy::I => {}
        }
    }
    (scope, live, handles)
}

/// `e` binds variables of its own.
fn has_let(e: &Core) -> bool {
    e.any(&mut |e| if matches!(e, Core::Let(..)) { Some(true) } else { None })
}

/// Required value form; tuple leaves may contain moved array slots.
#[derive(Clone, Copy)]
enum Expected<'a> {
    Int,
    Array,
    Tuple(&'a Shape),
}

struct Chk<'m> {
    why: Option<String>,
    sigs: &'m [Option<Sig>],
    scope: Scope,
    ok: bool,
    /// owned array slots alive on the current path
    live: std::collections::BTreeSet<u64>,
    /// borrowed params this body consumes (they must become owned)
    demote: Vec<u32>,
    /// borrowed alias -> the parameter it names
    root: HashMap<u32, u32>,
    /// constructor handles (owned: true): params and the fields a match binds
    hvars: HashMap<u32, bool>,
    /// occurrences of each variable in the body (an owned handle has one)
    uses: crate::Cnt,
}

impl<'m> Chk<'m> {
    /// An owned handle's one use (a match or a move); anything else leaks or frees twice.
    fn once(&self, h: u32) -> bool {
        self.uses.get(&h).copied().unwrap_or(0) == 1
    }

    /// A tuple returned by `fid` has the layout its callers read: the
    /// inferred result layout (flat when inference found none).
    fn ret_shape(&mut self, e: &Core, fid: u32, sh: &Shape) {
        if rshape(fid, sh.width()) != *sh {
            self.fail(e);
        }
    }

    /// A tuple variable used whole (returned, aliased, joined): its
    /// components are copied, which ownership allows for ints only.
    fn whole(&mut self, e: &Core, t: u32) {
        if self.scope.tarr.get(&t).is_some_and(|m| m.iter().any(|a| *a)) {
            self.fail(e);
        }
    }

    fn fail(&mut self, e: &Core) {
        if self.why.is_none() {
            self.why = Some(format!("{:.160}", format!("{:?}", e)));
        }
        self.ok = false;
    }

    /// An array read in place: must name a live slot or a borrowed var.
    /// (Every array in native code holds ints: parameters are typed
    /// `Arr(true)` at the boundary, and inside, arrays are only built and
    /// written with int-checked values.)
    fn read(&mut self, e: &Core) {
        match e {
            Core::Var(v) if self.scope.arrs.get(v) == Some(&true) => {}
            _ => match self.scope.slot(e) {
                Some(k) if self.live.contains(&k) => {}
                _ => self.fail(e),
            },
        }
    }

    /// Consume slot `k` (an owned array moves on).
    fn consume(&mut self, e: &Core, k: u64) {
        if let Core::Var(v) = e {
            if self.scope.arrs.get(v) == Some(&true) {
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
        self.check(e, Expected::Array)
    }

    /// Check both arms of an `if` from the same live set; true when both
    /// end in the same set.
    fn arms<R>(&mut self, c: &Core, x: &Core, y: &Core, mut f: impl FnMut(&mut Self, &Core) -> R) -> (R, R, bool) {
        self.expr(c);
        // each arm's bindings end with it
        let scope = (self.scope.clone(), self.root.clone());
        let before = self.live.clone();
        let a = f(self, x);
        (self.scope, self.root) = scope.clone();
        let after = std::mem::replace(&mut self.live, before);
        let b = f(self, y);
        (self.scope, self.root) = scope;
        let same = self.live == after;
        (a, b, same)
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
        if self.scope.array(self.sigs, e) {
            self.aexpr(e)
        } else {
            self.expr(e);
            None
        }
    }

    /// `e` is a plain i64-valued expression.
    fn expr(&mut self, e: &Core) {
        self.check(e, Expected::Int);
    }

    /// All native value forms share call, branch and let scoping. Array
    /// slots move only after the enclosing operation evaluates its inputs.
    fn check(&mut self, e: &Core, expected: Expected<'_>) -> Option<u64> {
        if !self.ok {
            return None;
        }
        if matches!(expected, Expected::Array) {
            if let Some(k) = self.scope.slot(e) {
                return Some(k);
            }
        }
        match (e, expected) {
            (Core::If(c, x, y), _) => {
                let same = self.arms(c, x, y, |s, arm| {
                    let d = s.check(arm, expected);
                    s.settle(arm, d);
                }).2;
                let masks_agree = match expected {
                    Expected::Tuple(sh) => self.scope.mask(self.sigs, x, sh.width())
                        == self.scope.mask(self.sigs, y, sh.width()),
                    _ => true,
                };
                if !same || !masks_agree {
                    self.fail(e);
                }
            }
            (Core::Let(x, r, b), _) => {
                self.bind(*x, r);
                let d = self.check(b, expected);
                self.settle(b, d);
            }
            (Core::Call(g, args), _) => {
                let accepts = self.sigs[*g as usize].as_ref().is_some_and(|sig| match expected {
                    Expected::Int => sig.ret == Kind::S1 && sig.ra.first() != Some(&true),
                    Expected::Array => sig.ret == Kind::S1 && sig.ra.first() == Some(&true),
                    Expected::Tuple(sh) => sig.ret == Kind::SK(sh.width()) && rshape(*g, sh.width()) == *sh,
                });
                if accepts {
                    self.args(*g, args);
                } else {
                    self.fail(e);
                }
            }
            (Core::Prim(Prim::ArrNew, xs), Expected::Array) => {
                self.expr(&xs[0]);
                self.expr(&xs[1]);
            }
            (Core::Prim(Prim::ArrSet, xs), Expected::Array) => {
                let d = self.aexpr(&xs[0]);
                self.expr(&xs[1]);
                self.expr(&xs[2]);
                self.settle(&xs[0], d);
            }
            (Core::Num(_), Expected::Int) => {}
            (Core::Prim(Prim::ArrGet, xs), Expected::Int) => {
                self.read(&xs[0]);
                self.expr(&xs[1]);
            }
            (Core::Prim(Prim::ArrLen, xs), Expected::Int) => self.read(&xs[0]),
            (Core::Prim(p, xs), Expected::Int) if p.is_f32() => xs.iter().for_each(|x| self.expr(x)),
            (Core::Var(i), Expected::Int) if self.scope.arrs.contains_key(i) || self.scope.tvars.contains_key(i) || self.hvars.contains_key(i) => self.fail(e),
            (Core::Var(_), Expected::Int) => {}
            (Core::Op2(_, a, b) | Core::Cmp(_, a, b), Expected::Int) => {
                self.expr(a);
                self.expr(b);
            }
            (Core::Proj(..), Expected::Int) if self.scope.slot(e).is_some() => self.fail(e),
            (Core::Proj(b, i), Expected::Int) if matches!(&**b, Core::Var(t) if self.scope.tvars.get(t).is_some_and(|s| s.0.get(*i) == Some(&None))) => {}
            (Core::Var(t), Expected::Tuple(sh)) if self.scope.tvars.get(t) == Some(sh) => self.whole(e, *t),
            (Core::Proj(..), Expected::Tuple(sh)) => self.targ(e, sh),
            (Core::Tuple(items), Expected::Tuple(sh)) if sh.is_flat() && items.len() == sh.width() => self.items(items),
            (Core::Tuple(_), Expected::Tuple(sh)) => self.targ(e, sh),
            _ => self.fail(e),
        }
        None
    }

    /// Evaluate tuple leaves before any of their owned slots move.
    fn items(&mut self, items: &[Core]) {
        let moved: Vec<(usize, u64)> = items.iter().enumerate().filter_map(|(j, it)| self.any(it).map(|d| (j, d))).collect();
        for (j, d) in moved {
            self.consume(&items[j], d);
        }
    }

    /// Check call arguments against the callee's parameter types.
    fn args(&mut self, g: u32, args: &[Core]) {
        let ptys = self.sigs[g as usize].as_ref().map(|s| s.params.clone()).unwrap_or_default();
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
                    lent.extend(self.scope.slot(a));
                }
                PTy::H if matches!(a, Core::Var(x) if self.hvars.get(x) == Some(&false)) => {}
                PTy::O if matches!(a, Core::Var(x) if self.hvars.get(x) == Some(&true) && self.once(*x)) => {}
                PTy::H | PTy::O => self.fail(a),
                PTy::I if self.scope.array(self.sigs, a) => self.fail(a),
                PTy::I => self.expr(a),
                PTy::T(k) => self.targ(a, &pshape(g, j, *k)),
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

    /// A tuple argument of layout `sh` (no arrays: its leaves are ints).
    fn targ(&mut self, a: &Core, sh: &Shape) {
        match a {
            Core::Var(t) if self.scope.tvars.get(t) == Some(sh) => self.whole(a, *t),
            Core::Proj(b, i) if matches!(&**b, Core::Var(t) if self.scope.tvars.get(t).and_then(|s| s.0.get(*i)) == Some(&Some(sh.clone()))) => {
                if let Core::Var(t) = &**b {
                    self.whole(a, *t);
                }
            }
            Core::Tuple(items) if items.len() == sh.0.len() => {
                for (it, c) in items.iter().zip(&sh.0) {
                    match c {
                        None => self.expr(it),
                        Some(sub) => self.targ(it, sub),
                    }
                }
            }
            _ => self.fail(a),
        }
    }

    /// A let binding: a plain scalar RHS, the SK-destructure idiom, or a
    /// tuple-valued expression (a join point: `if`/`match` arms that each
    /// yield a tuple of the variables they assign) bound as components.
    fn bind(&mut self, x: u32, r: &Core) {
        // The RHS is checked in the outer scope; its own bindings end
        // with it, and x then shadows the outer binding.
        let outer = has_let(r).then(|| (self.scope.clone(), self.root.clone()));
        let bound = self.scope.bound(self.sigs, r);
        match &bound {
            Bound::Tup(sh, _) => self.tuple_expr(r, sh),
            Bound::Owned => {
                let d = self.aexpr(r);
                self.settle(r, d);
            }
            Bound::Lent(_) => {}
            Bound::Scalar => self.expr(r),
        }
        if let Some((scope, root)) = outer {
            (self.scope, self.root) = (scope, root);
        }
        self.hvars.remove(&x);
        if let Bound::Lent(y) = &bound {
            self.root.insert(x, self.root.get(y).copied().unwrap_or(*y));
        }
        self.scope.bind(x, bound, &mut self.live);
    }

    /// A tuple-valued expression of layout `sh`.
    fn tuple_expr(&mut self, e: &Core, sh: &Shape) {
        self.check(e, Expected::Tuple(sh));
    }

    /// Tail position; returns the tail kind (S1 / SK(k)) with its array
    /// mask, or sets !ok. Owned arrays still alive at a tail are freed there
    /// by the emitter.
    fn tail(&mut self, e: &Core, fid: u32) -> (Kind, Vec<bool>) {
        if !self.ok {
            return (Kind::No, vec![]);
        }
        match e {
            Core::Tuple(_) | Core::Var(_) | Core::Proj(..) if tuple_kind(self.sigs, &self.scope.tvars, e).is_some() => {
                let sh = tuple_kind(self.sigs, &self.scope.tvars, e).unwrap();
                self.ret_shape(e, fid, &sh);
                let mask = self.scope.mask(self.sigs, e, sh.width());
                self.tuple_expr(e, &sh);
                (Kind::SK(sh.width()), mask)
            }
            Core::Let(x, r, b) => {
                self.bind(*x, r);
                self.tail(b, fid)
            }
            Core::Match(sc, arms) => {
                let owned = match &**sc {
                    Core::Var(h) => match self.hvars.get(h) {
                        Some(&o) if !o || self.once(*h) => o,
                        _ => {
                            self.fail(e);
                            return (Kind::No, vec![]);
                        }
                    },
                    _ => {
                        self.fail(e);
                        return (Kind::No, vec![]);
                    }
                };
                let scope = (self.scope.clone(), self.root.clone(), self.live.clone(), self.hvars.clone());
                let mut kind: Option<(Kind, Vec<bool>)> = None;
                for (ctor, binders, body) in arms.iter().filter(|a| a.0 != UNREACHABLE_CTOR) {
                    (self.scope, self.root, self.live, self.hvars) = scope.clone();
                    for (j, x) in binders.iter().enumerate() {
                        match field_ty(*ctor, j) {
                            crate::ty::Ty::Int => {}
                            crate::ty::Ty::Adt(_) if !owned || self.uses.get(x).copied().unwrap_or(0) <= 1 => {
                                self.hvars.insert(*x, owned);
                            }
                            _ => self.fail(e),
                        }
                    }
                    let k = self.tail(body, fid);
                    kind = match kind {
                        None => Some(k),
                        Some(prev) if prev == k || k.0 == Kind::No => Some(prev),
                        Some(prev) if prev.0 == Kind::No => Some(k),
                        Some(_) => {
                            self.fail(e);
                            Some((Kind::No, vec![]))
                        }
                    };
                }
                self.hvars = scope.3;
                kind.unwrap_or((Kind::No, vec![]))
            }
            Core::If(c, x, y) => {
                let (a, b, _) = self.arms(c, x, y, |s, e| s.tail(e, fid));
                if a == b || b.0 == Kind::No {
                    a
                } else if a.0 == Kind::No {
                    b
                } else {
                    self.fail(e);
                    (Kind::No, vec![])
                }
            }
            Core::Call(g, args) => {
                match &self.sigs[*g as usize] {
                    Some(sig) => {
                        let sig = sig.clone();
                        if let Kind::SK(k) = sig.ret {
                            self.ret_shape(e, fid, &rshape(*g, k));
                        }
                        self.args(*g, args);
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
                if self.scope.array(self.sigs, other) {
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
pub(crate) fn calls_fn(e: &Core, g: u32) -> bool {
    e.any(&mut |e| if matches!(e, Core::Call(h, _) if *h == g) { Some(true) } else { None })
}

/// `fid` is reached again through another function's body (a cycle).
pub(crate) fn recursive_via_others(m: &CoreModule, fid: u32) -> bool {
    crate::callees(&m.fns[fid as usize].body).into_iter().any(|g| g != fid && crate::reaches(m, g, fid))
}

/// `e` calls a function other than `g` that does not always inline.
fn calls_other_real(m: &CoreModule, e: &Core, g: u32) -> bool {
    e.any(&mut |e| match e {
        Core::Call(h, _) if *h != g && !crate::inline_attr(&m.fns[*h as usize].body) => Some(true),
        _ => None,
    })
}

/// The prelude helper of a binary32 primitive.
pub(crate) fn f32_fn(p: Prim) -> &'static str {
    use Prim::*;
    match p {
        F32Add => "f32_add",
        F32Sub => "f32_sub",
        F32Mul => "f32_mul",
        F32Div => "f32_div",
        F32Sqrt => "f32_sqrt",
        F32Lt => "f32_lt",
        F32Le => "f32_le",
        F32FromU32 => "f32_from_u32",
        F32ToU32 => "f32_to_u32",
        _ => unreachable!(),
    }
}

/// An expression that can be evaluated as a value join: no calls and no
/// array writes or allocations (reads are fine).
fn join_arm(e: &Core) -> bool {
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
/// The layout of a tuple-valued expression (`tv`: the tuple variables in
/// scope, each held as its leaves).
fn tuple_kind(sigs: &[Option<Sig>], tv: &HashMap<u32, Shape>, e: &Core) -> Option<Shape> {
    match e {
        Core::Tuple(items) => Some(Shape(items.iter().map(|it| tuple_kind(sigs, tv, it)).collect())),
        Core::Var(t) => tv.get(t).cloned(),
        Core::Proj(b, i) => match &**b {
            Core::Var(t) => tv.get(t).and_then(|s| s.0.get(*i).cloned().flatten()),
            _ => None,
        },
        Core::If(_, x, y) => {
            let (a, b) = (tuple_kind(sigs, tv, x), tuple_kind(sigs, tv, y));
            if a == b {
                a
            } else {
                None
            }
        }
        Core::Let(x, r, b) => {
            let mut inner = tv.clone();
            match tuple_kind(sigs, tv, r) {
                Some(sh) => inner.insert(*x, sh),
                None => inner.remove(x),
            };
            tuple_kind(sigs, &inner, b)
        }
        Core::Call(g, _) => match &sigs[*g as usize] {
            Some(Sig { ret: Kind::SK(k), .. }) => Some(rshape(*g, *k)),
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
        Core::Lam(..) | Core::App(..) => {}
        _ => e.kids().into_iter().for_each(|k| proj_shape(k, p, bare, max)),
    }
}

thread_local! {
    /// The inferred layouts of tuple parameters and results (`ty::Types`).
    static SHAPES: std::cell::RefCell<(Vec<Vec<Option<Shape>>>, Vec<Option<Shape>>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
    /// Scalar signatures of the module being emitted, for direct calls
    /// from dive code (`native_sig`).
    pub(crate) static SIGS: std::cell::RefCell<Vec<Option<Sig>>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Native functions a dive caller must not call natively: proven folds (their
    /// bridge splits the range) and functions whose calls reach one.
    pub(crate) static DUAL: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
    /// `LEAF[g]`: native `g` is call-free and loop-free. It settles no fuel
    /// itself; its one unit is counted at the call site (a register
    /// increment in native callers), so fuel still measures work.
    pub(crate) static LEAF: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
    /// `BRIDGE_LIVE[g]`: native `g` can be reached through its dive bridge
    /// (it is the entry, a fold, or has a non-native caller).
    pub(crate) static BRIDGE_LIVE: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
    /// `CTX[g]`: native `g` takes the worker context (it touches arrays,
    /// itself or through a callee); the rest keep their argument registers.
    pub(crate) static CTX: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
    /// (ctor, field) -> inferred type, for the fields a native match binds.
    static FIELDS: std::cell::RefCell<Vec<Vec<crate::ty::Ty>>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Unboxed constructors (ctor -> tag slot), for native match dispatch.
    pub(crate) static UNBOX: std::cell::RefCell<HashMap<u32, u8>> = std::cell::RefCell::new(HashMap::new());
}

fn field_ty(ctor: u32, j: usize) -> crate::ty::Ty {
    FIELDS.with(|f| f.borrow().get(ctor as usize).and_then(|fs| fs.get(j).copied()).unwrap_or(crate::ty::Ty::Dyn))
}

/// Entry `g` of a per-function flag table (`d` past its end).
pub(crate) fn flag(t: &'static std::thread::LocalKey<std::cell::RefCell<Vec<bool>>>, g: u32, d: bool) -> bool {
    t.with(|l| l.borrow().get(g as usize).copied().unwrap_or(d))
}

/// The context argument of a call to native `g` (`"ctx, "` or nothing).
pub(crate) fn ctx_arg(g: u32) -> &'static str {
    if flag(&CTX, g, true) {
        "ctx, "
    } else {
        ""
    }
}

/// Which native functions need the context: array prims or array params
/// or results, or a call to one that does (fixpoint).
pub(crate) fn needs_ctx(m: &CoreModule, sigs: &[Option<Sig>]) -> Vec<bool> {
    let n = m.fns.len();
    let mut calls = vec![Vec::new(); n];
    let need: Vec<bool> = (0..n)
        .map(|f| {
            let Some(sig) = &sigs[f] else { return true };
            let mut p = false;
            m.fns[f].body.walk(&mut |e| match e {
                Core::Prim(pr, _) if !pr.is_f32() => p = true,
                Core::Match(..) => p = true,
                Core::Call(g, _) => calls[f].push(*g),
                _ => {}
            });
            p || sig.params.iter().any(|t| matches!(t, PTy::A | PTy::B | PTy::H | PTy::O)) || sig.ra.iter().any(|a| *a)
        })
        .collect();
    crate::fixpoint(need, |f, s| calls[f].iter().any(|g| s[*g as usize]))
}

pub(crate) fn is_leaf(g: u32) -> bool {
    flag(&LEAF, g, false)
}

/// The signature of `g` when dive code may call its native form directly:
/// scalar-lowered, all-int parameters, and never suspending (a forking
/// scalar function keeps a dive form of its own and is not included).
pub(crate) fn native_sig(g: u32) -> Option<Sig> {
    if flag(&DUAL, g, false) {
        return None;
    }
    SIGS.with(|s| s.borrow().get(g as usize).cloned().flatten())
        .filter(|s| s.params.iter().all(|p| *p == PTy::I) && s.ra.iter().all(|a| !a))
}

/// The layout of native tuple parameter `p` of `g` with `k` leaves: the
/// inferred one, or flat when inference found no tuple of that width.
fn pshape(g: u32, p: usize, k: usize) -> Shape {
    SHAPES.with(|s| s.borrow().0.get(g as usize).and_then(|ps| ps.get(p).cloned().flatten())).filter(|s| s.width() == k).unwrap_or_else(|| Shape::flat(k))
}

/// `g` returns a flat tuple (what a dive-form caller destructures).
pub(crate) fn flat_ret(g: u32, k: usize) -> bool {
    rshape(g, k).is_flat()
}

/// The layout of `g`'s tuple result with `k` leaves (as `pshape`).
fn rshape(g: u32, k: usize) -> Shape {
    SHAPES.with(|s| s.borrow().1.get(g as usize).cloned().flatten()).filter(|s| s.width() == k).unwrap_or_else(|| Shape::flat(k))
}

/// A tuple param's component count cannot be read off the body alone (a
/// caller may pass a wider tuple); seed with maxproj+1 and demote on caller
/// mismatch. Bare use of a param whose callers pass tuples also demotes.
/// `bor[f][p]`: the dive side lends parameter `p` (a constructor value is
/// matched natively only then; an owned one is consumed cell by cell).
pub(crate) fn classify(m: &CoreModule, tys: &crate::ty::Types, bor: &[Vec<bool>]) -> Vec<Option<Sig>> {
    use crate::ty::Ty;
    SHAPES.with(|s| *s.borrow_mut() = (tys.pshape.clone(), tys.rshape.clone()));
    FIELDS.with(|f| *f.borrow_mut() = tys.field.clone());
    let n = m.fns.len();
    // a value native code cannot represent (a non-int array at the
    // boundary, a float anywhere) rules the function out
    let forbid: Vec<bool> = (0..n)
        .map(|fi| {
            tys.params[fi].iter().chain(std::iter::once(&tys.ret[fi])).any(|t| matches!(t, Ty::Arr(false)))
                || tys.locals[fi].iter().any(|t| matches!(t, Ty::Flo))
                // a tuple parameter holding more than ints (native code holds
                // a parameter's leaves as i64s)
                || tys.params[fi].iter().zip(&tys.pshape[fi]).any(|(t, sh)| matches!(t, Ty::Tup(_)) && sh.is_none())
                // a parameter holding values of conflicting types (the
                // native form would read a non-int as an int)
                || tys.pmixed[fi].iter().any(|m| *m)
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
                    match tys.params[fi].get(p as usize) {
                        Some(Ty::Arr(true)) => return PTy::B, // optimistic: demoted when consumed
                        Some(Ty::Adt(_)) => return if bor[fi][p as usize] { PTy::H } else { PTy::O },
                        // a tuple with an int layout (type inference): exact,
                        // and the parameter may then be used whole; a tuple
                        // holding other values takes the width its body reads
                        Some(Ty::Tup(_)) if tys.pshape[fi][p as usize].is_some() => {
                            return PTy::T(tys.pshape[fi][p as usize].as_ref().map_or(0, Shape::width));
                        }
                        _ => {}
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
                Some(Ty::Tup(_)) if tys.rshape[fi].is_some() => {
                    let k = tys.rshape[fi].as_ref().map_or(0, Shape::width);
                    (Kind::SK(k), vec![false; k])
                }
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
        let (scope, live, hvars) = param_bindings(fid as u32, params);
        let mut c = Chk {
            why: None,
            sigs,
            scope,
            ok: true,
            live,
            demote: Vec::new(),
            root: HashMap::new(),
            hvars,
            uses: {
                let mut u = crate::Cnt::new();
                crate::cnt_expr(&m.fns[fid].body, &mut u);
                u
            },
        };
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
    // MITHRIL_WHY_BOXED: why each function has no native form (a diagnostic)
    if std::env::var_os("MITHRIL_WHY_BOXED").is_some() {
        for fid in (0..n).filter(|&f| sigs[f].is_none()) {
            let why = if forbid[fid] { "a float, boxed array, non-int tuple or mixed-type value".to_string() } else { whys[fid].clone().unwrap_or_else(|| "signature did not settle".into()) };
            eprintln!("boxed {}: {why} (params {:?}, result {:?})", m.fns[fid].name, tys.params[fid], tys.ret[fid]);
        }
    }
    sigs
}

// ---------------------------------------------------------------- emission

struct Sem<'m> {
    sigs: &'m [Option<Sig>],
    ranges: crate::range::Ranges,
    /// the Op2 about to be emitted only has its low 32 bits observed
    low32: bool,
    tmp: u32,
    scope: Scope,
    /// owned array slots alive on the current path (as in `Chk`)
    live: std::collections::BTreeSet<u64>,
    /// the function counts fuel (it loops or calls); a call-free, loop-free
    /// body does bounded work and settles none
    fuel: bool,
    /// array value name -> the local holding its length (read once where
    /// the array enters; a write or a move keeps it), so bounds checks
    /// compare against a register instead of reloading the header
    lens: HashMap<String, String>,
    /// length locals are used in this function (see `scalar_fn`)
    use_lens: bool,
    /// constructor handles, owned: true (see `Chk::hvars`)
    hvars: HashMap<u32, bool>,
    /// this function, and the `array_new` being bound is written in full
    /// by a fill before any read (`uninit_lets`)
    fid: u32,
    uninit: bool,
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
            b.push(crate::lir::work_fuel(v("fl")));
        }
    }

    fn fresh(&mut self) -> String {
        self.tmp += 1;
        format!("s{}", self.tmp)
    }

    fn temp(&mut self, ty: Ty, e: E, b: &mut Vec<S>) -> E {
        crate::value::bind(self.fresh(), ty, e, b)
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

    /// an element read, through the held length when there is one
    fn arr_get(&self, a: &str, i: E) -> E {
        match self.acc(a) {
            Some(l) => p("arr_get_n", vec![as_u(v(a)), v(l), i]),
            None => p("arr_get_r", vec![as_u(v(a)), i]),
        }
    }

    /// The length local of array value `a` when known.
    fn acc(&self, a: &str) -> Option<String> {
        self.lens.get(a).cloned()
    }

    /// Name of an array read in place (a live slot or a borrowed var).
    fn rd(&self, e: &Core) -> String {
        match e {
            Core::Var(x) => vn(*x),
            _ => slot_name(self.scope.slot(e).expect("array read of a non-slot in scalar emission")),
        }
    }

    /// An array-valued expression in a consuming position; a slot operand
    /// is returned by name with its key for deferred consumption.
    fn aval(&mut self, e: &Core, b: &mut Vec<S>) -> (E, Option<u64>) {
        if let Some(k) = self.scope.slot(e) {
            return (v(slot_name(k)), Some(k));
        }
        let t = self.fresh();
        match e {
            Core::Prim(Prim::ArrNew, xs) => {
                let n = self.val(&xs[0], b);
                let alloc = if std::mem::take(&mut self.uninit) { p("arr_new_raw_uninit", vec![n]) } else { p("arr_new_raw", vec![n, self.val(&xs[1], b)]) };
                b.push(let_(&t, Ty::I64, cast(alloc, Ty::I64)));
                if self.use_lens {
                    b.push(let_(format!("l_{t}"), Ty::Usize, p("arr_len_of", vec![as_u(v(&t))])));
                    self.lens.insert(t.clone(), format!("l_{t}"));
                }
            }
            Core::Prim(Prim::ArrSet, xs) => {
                let (a, d) = self.aval(&xs[0], b);
                let i = self.val(&xs[1], b);
                let x = self.val(&xs[2], b);
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
                self.join(cd, x, y, b, |_| vec![t.clone()], |s, e, bb| vec![s.aval_now(e, bb)]);
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                return (self.aval_now(bo, b), None);
            }
            _ => unreachable!("non-array Core in scalar array emission"),
        }
        (v(t), None)
    }

    /// Both arms of a value `if` into the locals `names` makes (after the
    /// condition), from the same live set and lengths; x's live set
    /// survives (the arms agree, see Chk).
    fn join(&mut self, cd: &Core, x: &Core, y: &Core, b: &mut Vec<S>, names: impl FnOnce(&mut Self) -> Vec<String>, mut arm: impl FnMut(&mut Self, &Core, &mut Vec<S>) -> Vec<E>) -> Vec<String> {
        let ec = self.val(cd, b);
        let names = names(self);
        let (live0, lens0) = (self.live.clone(), self.lens.clone());
        // each arm's bindings end with it
        let scope = self.scope.clone();
        let mut bx = Vec::new();
        for (n, e) in names.iter().zip(arm(self, x, &mut bx)) {
            bx.push(set(n, e));
        }
        let after = std::mem::replace(&mut self.live, live0);
        self.lens = lens0.clone();
        self.scope = scope.clone();
        let mut by = Vec::new();
        for (n, e) in names.iter().zip(arm(self, y, &mut by)) {
            by.push(set(n, e));
        }
        (self.live, self.lens) = (after, lens0);
        self.scope = scope;
        b.extend(names.iter().map(|n| S::Decl(n.clone(), Ty::I64)));
        b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bx, by));
        names
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
        if self.scope.array(self.sigs, e) {
            self.aval(e, b)
        } else {
            (self.val(e, b), None)
        }
    }

    /// The call `s_g(ctx?, fuel, args..)` with its fuel accounting.
    fn raw_call(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> E {
        let es = self.call_args(g, args, b);
        if is_leaf(g) {
            b.push(set("fl", bin(Bop::Add, v("fl"), i64_(1)))); // a leaf's unit, counted here
        }
        let mut a = vec![v("fuel")];
        a.extend(es);
        E::Call { f: format!("s_{g}"), ctx: !ctx_arg(g).is_empty(), args: a }
    }

    /// A single-value call's result in this function's representation.
    fn call(&mut self, g: u32, args: &[Core], b: &mut Vec<S>) -> E {
        self.raw_call(g, args, b)
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
        rs.iter().map(v).collect()
    }

    /// Free the owned arrays still alive at a path's end.
    fn drop_live(&self, b: &mut Vec<S>) {
        for k in &self.live {
            b.push(free(as_u(v(slot_name(*k)))));
        }
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
                PTy::H | PTy::O => es.push(self.val(a, b)),
                PTy::I => es.push(self.val(a, b)),
                PTy::T(_) => es.extend(self.tval(a, b)),
            }
        }
        for k in moved {
            self.live.remove(&k);
        }
        es
    }

    fn bind(&mut self, x: u32, r: &Core, b: &mut Vec<S>) {
        let outer = has_let(r).then(|| (self.scope.clone(), self.lens.clone()));
        let bound = self.scope.bound(self.sigs, r);
        let es = match &bound {
            Bound::Tup(_, _) => self.tval(r, b),
            Bound::Lent(y) => vec![v(vn(*y))],
            Bound::Owned => {
                self.uninit = crate::uninit_let(self.fid, x);
                let e = self.aval_now(r, b);
                self.uninit = false;
                vec![e]
            }
            Bound::Scalar => {
                self.low32 = self.ranges.masked32(x);
                vec![self.val(r, b)]
            }
        };
        // Carry an array's cached length across its move or borrowed alias.
        let len = matches!(bound, Bound::Lent(_) | Bound::Owned).then(|| self.acc(&name_of(&es[0]))).flatten();
        if let Some((scope, lens)) = outer {
            (self.scope, self.lens) = (scope, lens);
        }
        let tuple = matches!(bound, Bound::Tup(..));
        self.scope.bind(x, bound, &mut self.live);
        for (i, e) in es.into_iter().enumerate() {
            b.push(let_(if tuple { format!("q{x}_{i}") } else { vn(x) }, Ty::I64, e));
        }
        if let Some(l) = len {
            self.lens.insert(vn(x), l);
        }
    }

    /// A tuple-valued expression as its leaves, in order.
    fn tval(&mut self, e: &Core, b: &mut Vec<S>) -> Vec<E> {
        match e {
            Core::Tuple(items) => {
                let mut moved = Vec::new();
                let mut es: Vec<E> = Vec::new();
                for it in items {
                    if tuple_kind(self.sigs, &self.scope.tvars, it).is_some() {
                        es.extend(self.tval(it, b));
                    } else {
                        let (x, d) = self.anyval(it, b);
                        moved.extend(d);
                        es.push(x);
                    }
                }
                for k in moved {
                    self.live.remove(&k);
                }
                es
            }
            Core::If(cd, x, y) => {
                let k = tuple_kind(self.sigs, &self.scope.tvars, e).expect("tuple if").width();
                let names = |s: &mut Self| {
                    let t = s.fresh();
                    (0..k).map(|i| format!("{t}_{i}")).collect()
                };
                self.join(cd, x, y, b, names, |s, e, bb| s.tval(e, bb)).into_iter().map(v).collect()
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.tval(bo, b)
            }
            Core::Call(g, args) => self.call_tuple(*g, args, b),
            Core::Var(t) if self.scope.tvars.contains_key(t) => (0..self.scope.tvars[t].width()).map(|i| v(format!("q{t}_{i}"))).collect(),
            // a nested component: its leaves
            Core::Proj(base, i) => match &**base {
                Core::Var(t) => {
                    let sh = &self.scope.tvars[t];
                    let (off, w) = (sh.offset(*i), sh.0[*i].as_ref().map_or(1, Shape::width));
                    (off..off + w).map(|j| v(format!("q{t}_{j}"))).collect()
                }
                _ => unreachable!("non-idiom Proj in scalar tuple emission"),
            },
            _ => unreachable!("non-tuple Core in scalar tuple emission"),
        }
    }

    fn val(&mut self, e: &Core, b: &mut Vec<S>) -> E {
        let low32 = std::mem::take(&mut self.low32);
        match e {
            Core::Op2(..) | Core::Cmp(..) => self.plain_arith(e, low32, b),
            Core::Num(n) => i64_(*n),
            Core::Prim(Prim::ArrGet, xs) => {
                let a = self.rd(&xs[0]);
                let i = self.val(&xs[1], b);
                let get = self.arr_get(&a, i);
                self.temp(Ty::I64, cast(get, Ty::I64), b)
            }
            Core::Prim(pr, xs) if pr.is_f32() => {
                // the bit patterns go through plain
                let es: Vec<E> = xs.iter().map(|x| self.val(x, b)).collect();
                self.temp(Ty::I64, p(f32_fn(*pr), es), b)
            }
            Core::Prim(Prim::ArrLen, xs) => {
                let a = self.rd(&xs[0]);
                let l = self.acc(&a).map(v).unwrap_or_else(|| p("arr_len_of", vec![as_u(v(a))]));
                cast(l, Ty::I64)
            }
            Core::Var(i) => v(vn(*i)),
            Core::Proj(base, i) => match &**base {
                Core::Var(t) if self.scope.tvars.contains_key(t) => v(format!("q{t}_{}", self.scope.tvars[t].offset(*i))),
                _ => unreachable!("non-idiom Proj in scalar emission"),
            },
            Core::If(cd, x, y) if matches!((&**x, &**y), (Core::Num(_) | Core::Var(_), Core::Num(_) | Core::Var(_))) => {
                // a select of computed values (what if-conversion leaves):
                // mask arithmetic, so the backend cannot turn it back into
                // an unpredictable branch on a loop-carried chain
                let ec = self.val(cd, b);
                let (vx, vy) = (self.val(x, b), self.val(y, b));
                let t = self.fresh();
                let m = format!("{t}m");
                b.push(let_(&m, Ty::I64, E::Neg(Box::new(cast(bin(Bop::Ne, ec, i64_(0)), Ty::I64)))));
                let selected = bin(Bop::Or, bin(Bop::And, vx, v(&m)), bin(Bop::And, vy, bin(Bop::Xor, v(&m), i64_(-1))));
                crate::value::bind(t, Ty::I64, selected, b)
            }
            Core::If(cd, x, y) => v(self.join(cd, x, y, b, |s| vec![s.fresh()], |s, e, bb| vec![s.val(e, bb)]).remove(0)),
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                self.val(bo, b)
            }
            Core::Call(g, args) => {
                let call = self.call(*g, args, b);
                self.temp(Ty::I64, call, b)
            }
            _ => unreachable!("non-scalar Core in scalar emission"),
        }
    }

    /// Native arithmetic: 64-bit wrapping; masked 32-bit ops run in u32.
    fn plain_arith(&mut self, e: &Core, low32: bool, b: &mut Vec<S>) -> E {
        match e {
            Core::Op2(op, x, y) if low32 && crate::range::low32_closed(op) => {
                // only the low 32 bits are observed: compute in u32 (the
                // port's `& 0xFFFFFFFF` masks become free)
                self.low32 = true;
                let ex = self.val(x, b);
                self.low32 = *op != mithril_front::ast::BinOp::Shl;
                let ey = self.val(y, b);
                let body = arith(bin_code(op), cast(ex, Ty::U32), cast(ey, Ty::U32));
                self.temp(Ty::I64, cast(body, Ty::I64), b)
            }
            Core::Op2(op, x, y) => {
                self.low32 = crate::range::feeds_mask32(op, y);
                let ex = self.val(x, b);
                let ey = self.val(y, b);
                // ints are 64-bit: the machine's wrapping arithmetic is the semantics
                let body = arith(bin_code(op), ex, ey);
                self.temp(Ty::I64, body, b)
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
        let result = match e {
            // `let x = r in x` is `r`: a call bound only to be returned is a tail call
            Core::Let(x, r, bo) if matches!(**bo, Core::Var(y) if y == *x) && matches!(**r, Core::Call(..)) => {
                return self.tail(r, fid, lp, b);
            }
            Core::Let(x, r, bo) => {
                self.bind(*x, r, b);
                return self.tail(bo, fid, lp, b);
            }
            Core::If(..) if join_arm(e) && !self.scope.array(self.sigs, e) && tuple_kind(self.sigs, &self.scope.tvars, e).is_none_or(|sh| {
                self.scope.mask(self.sigs, e, sh.width()).iter().all(|a| !a)
            }) =>
            {
                // call-free, array-free arms: one value join and a single
                // return (per-arm returns each settle fuel, which keeps the
                // backend from turning a cheap unpredictable branch into a
                // select)
                match tuple_kind(self.sigs, &self.scope.tvars, e) {
                    Some(_) => E::Tup(self.tval(e, b)),
                    None => self.val(e, b),
                }
            }
            Core::If(cd, x, y) => {
                let ec = self.val(cd, b);
                let (live, scope, lens) = (self.live.clone(), self.scope.clone(), self.lens.clone());
                let mut bx = Vec::new();
                self.tail(x, fid, lp, &mut bx);
                (self.live, self.scope, self.lens) = (live, scope.clone(), lens.clone());
                let mut by = Vec::new();
                self.tail(y, fid, lp, &mut by);
                (self.scope, self.lens) = (scope, lens);
                b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bx, by));
                return;
            }
            Core::Match(sc, arms) => {
                let Core::Var(h) = &**sc else { unreachable!("native match on a non-handle") };
                let hv = as_u(v(vn(*h)));
                let owned = self.hvars.get(h) == Some(&true);
                let unbox = UNBOX.with(|u| u.borrow().clone());
                let saved = (self.live.clone(), self.scope.clone(), self.lens.clone(), self.hvars.clone());
                let mut bodies = Vec::new();
                for (ctor, binders, body) in arms {
                    (self.live, self.scope, self.lens, self.hvars) = saved.clone();
                    let mut ab = Vec::new();
                    if *ctor != UNREACHABLE_CTOR {
                        let used = crate::free_vars(body);
                        // each field as a port: read in place, or moved out of the consumed cells
                        let n = binders.len();
                        let ports: Vec<E> = if unbox.contains_key(ctor) {
                            binders.iter().map(|_| hv.clone()).collect()
                        } else {
                            let names: Vec<String> = (0..n).map(|j| format!("f{h}_{j}")).collect();
                            crate::value::fields(hv.clone(), *ctor, names, owned && n > 0, None, &mut ab)
                        };
                        for ((j, x), f) in binders.iter().enumerate().zip(ports) {
                            if !used.contains(x) {
                                if owned && !unbox.contains_key(ctor) {
                                    ab.push(free(f));
                                }
                                continue;
                            }
                            let val = match field_ty(*ctor, j) {
                                crate::ty::Ty::Adt(_) => {
                                    self.hvars.insert(*x, owned);
                                    cast(f, Ty::I64)
                                }
                                // an int moved out of a consumed cell may be a box
                                _ if owned && !unbox.contains_key(ctor) => p("take_i", vec![f]),
                                _ => crate::lir::as_i(f),
                            };
                            ab.push(let_(vn(*x), Ty::I64, val));
                        }
                        self.tail(body, fid, lp, &mut ab);
                    }
                    bodies.push(ab);
                }
                (self.scope, self.lens, self.hvars) = (saved.1, saved.2, saved.3);
                b.push(crate::seq::plan_arms(&hv, arms, &unbox, |i| bodies[i].clone()));
                return;
            }
            Core::Call(g, args) if *g == fid && lp => {
                let es = self.call_args(*g, args, b);
                for (i, ea) in es.iter().enumerate() {
                    b.push(let_(format!("n{i}"), Ty::I64, ea.clone()));
                }
                self.drop_live(b);
                let sig = self.sigs[fid as usize].clone().unwrap();
                let names = flat_names(&sig);
                let mut nl = Vec::new();
                for (pp, pt) in sig.params.iter().enumerate() {
                    if self.use_lens && matches!(pt, PTy::A | PTy::B) {
                        // es is flattened; array params occupy one slot each
                        let j = names.iter().position(|n| *n == vn(pp as u32)).unwrap();
                        let l = self.len_of(&name_of(&es[j]), b);
                        nl.push((pp, l));
                    }
                }
                for (i, slot) in names.into_iter().enumerate() {
                    b.push(set(slot, v(format!("n{i}"))));
                }
                for (pp, l) in nl {
                    b.push(set(format!("l_v{pp}"), v(l)));
                }
                b.push(S::Continue);
                return;
            }
            Core::Call(g, args) => {
                let call = match self.sigs[*g as usize].as_ref().map(|s| s.ret) {
                    Some(Kind::SK(_)) => {
                        let comps = self.call_tuple(*g, args, b);
                        E::Tup(comps)
                    }
                    _ => self.call(*g, args, b),
                };
                // nothing left to free after it: settle this frame's work first
                // and return the call itself, so a tail call stays a jump (the
                // callee charges its own work; the totals are unchanged)
                if self.live.is_empty() {
                    self.settle_fuel(b);
                    b.push(ret(call));
                    return;
                }
                // the call reads arrays lent to it: run it before they are freed
                self.temp(Ty::Infer, call, b)
            }
            Core::Tuple(_) | Core::Var(_) | Core::Proj(..) if tuple_kind(self.sigs, &self.scope.tvars, e).is_some() => {
                E::Tup(self.tval(e, b))
            }
            other => {
                if self.scope.array(self.sigs, other) { self.aval_now(other, b) } else { self.val(other, b) }
            }
        };
        self.drop_live(b);
        self.settle_fuel(b);
        b.push(ret(result));
    }
}

/// Local names of a signature's flattened parameters: v<p>, or q<p>_<i>
/// per tuple component.
fn flat_names(sig: &Sig) -> Vec<String> {
    let comps = |(pp, pt): (usize, &PTy)| match pt {
        PTy::T(k) => (0..*k).map(|i| format!("q{pp}_{i}")).collect(),
        _ => vec![vn(pp as u32)],
    };
    sig.params.iter().enumerate().flat_map(comps).collect()
}

/// Port/native boundary shared by the scalar dive and local region entries.
/// The signature owns conversion and borrowing; the result form chooses only
/// whether a tuple crosses the boundary as cells or as an aggregate.
pub(crate) fn port_bridge(fid: u32, sig: &Sig, bor: &[bool], ret_ty: Ty) -> Vec<S> {
    // bridge: unpack ports per param type, call, repack per return kind
    let mut unpack = Vec::new();
    let mut bargs: Vec<E> = vec![v("fuel")];
    let mut after = Vec::new();
    let conv = crate::lir::as_i;
    for (pp, pt) in sig.params.iter().enumerate() {
        let pv = v(vn(pp as u32));
        match pt {
            // an owned int port may be a box: released after the read
            PTy::I if !bor[pp] => bargs.push(p("take_i", vec![pv])),
            PTy::I => bargs.push(conv(pv)),
            // array ports pass as their bits; the dive side's borrow mode
            // for this param may differ from the native one
            PTy::A => {
                if bor[pp] {
                    unpack.push(let_(vn(pp as u32), Ty::U64, c("dup_val", vec![pv.clone()])));
                }
                // native code writes owned arrays without refcount checks
                unpack.push(let_(vn(pp as u32), Ty::U64, c("arr_own", vec![pv.clone()])));
                bargs.push(cast(pv, Ty::I64));
            }
            PTy::B | PTy::H => {
                if !bor[pp] {
                    after.push(free(pv.clone()));
                }
                bargs.push(cast(pv, Ty::I64));
            }
            PTy::O => bargs.push(cast(pv, Ty::I64)),
            PTy::T(k) => {
                // the port's leaves, nested tuples read through
                fn leaves(src: E, sh: &Shape, name: &str, conv: &dyn Fn(E) -> E, out: &mut Vec<S>, args: &mut Vec<E>) {
                    for (i, comp) in sh.0.iter().enumerate() {
                        let (n, f) = (format!("{name}_{i}"), c("field", vec![src.clone(), usize_(i)]));
                        match comp {
                            None => {
                                out.push(let_(&n, Ty::I64, conv(f)));
                                args.push(v(n));
                            }
                            Some(sub) => {
                                out.push(let_(&n, Ty::U64, f));
                                leaves(v(&n), sub, &n, conv, out, args);
                            }
                        }
                    }
                }
                leaves(pv.clone(), &pshape(fid, pp, *k), &format!("a{pp}"), &conv, &mut unpack, &mut bargs);
                if !bor[pp] {
                    unpack.push(free(pv));
                }
            }
        }
    }
    let pack = |i: usize, r: E| if sig.ra.get(i).copied().unwrap_or(false) { as_u(r) } else { crate::lir::num(r) };
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
            // one tuple cell per level of the result's layout, inner ones
            // first (each allocation takes the context)
            fn cells(sh: &Shape, next: &mut usize, leaf: &dyn Fn(usize) -> E, out: &mut Vec<S>) -> E {
                let mut fs = Vec::new();
                for comp in &sh.0 {
                    match comp {
                        None => {
                            *next += 1;
                            fs.push(leaf(*next - 1));
                        }
                        Some(sub) => {
                            let inner = cells(sub, next, leaf, out);
                            let n = format!("rc{}", out.len());
                            out.push(let_(&n, Ty::U64, inner));
                            fs.push(v(n));
                        }
                    }
                }
                c("mk_con", vec![u16_(0xFFF), E::Slice(fs)])
            }
            let outer = if matches!(ret_ty, Ty::ResArr(_)) { E::Arr(comps.iter().enumerate().map(|(i, n)| pack(i, v(n))).collect()) } else {
                cells(&rshape(fid, k), &mut 0, &|i| pack(i, v(&comps[i])), &mut bridge_body)
            };
            bridge_body.push(ret(ok(outer)));
        }
        Kind::No => unreachable!(),
    }
    bridge_body
}

/// The native form `s_<fid>` plus the bridging dive form `d_<fid>`.
/// `bor[fid][p]` = param p is borrowed (bridge must not free a tuple arg's
/// spine; owned tuple args are freed after unpacking — components are NUMs,
/// so only the spine cells matter).
pub(crate) fn scalar_fn(m: &CoreModule, fid: u32, sigs: &[Option<Sig>], bor: &[Vec<bool>], bridge: bool) -> Vec<FnDef> {
    let f = &m.fns[fid as usize];
    let ar = f.arity;
    // a loop only where the body calls itself (`self_tail_rec` also holds,
    // vacuously, for a function with no self call)
    let lp = f.self_tail_rec && calls_fn(&f.body, fid);
    let sig = sigs[fid as usize].as_ref().unwrap().clone();
    let mut params: Vec<(String, Ty)> = vec![("fuel".into(), Ty::RefI64)];
    params.extend(flat_names(&sig).into_iter().map(|n| (n, Ty::I64)));
    let ret_ty = match sig.ret {
        Kind::S1 => Ty::I64,
        Kind::SK(k) => Ty::Tup(k),
        Kind::No => unreachable!(),
    };
    let (scope, live, hvars) = param_bindings(fid, &sig.params);
    let mut sem = Sem {
        sigs,
        ranges: crate::range::Ranges::of(&f.body),
        low32: false,
        tmp: 0,
        scope,
        live,
        fuel: !is_leaf(fid),
        lens: HashMap::new(),
        use_lens: false,
        hvars,
        fid,
        uninit: false,
    };
    // loop-carried array params keep their length in a loop variable;
    // elsewhere a length is read at its first use
    // Length locals pay where the backend cannot keep a length itself: a
    // loop that writes an array and also accesses another one (the write
    // may alias the other's header, forcing a reload per access). With one
    // array, or only reads, the header load is hoisted already, and a
    // register length only obstructs the backend (e.g. vectorization).
    let arr_ps: Vec<usize> = sig.params.iter().enumerate().filter(|(_, t)| matches!(t, PTy::A | PTy::B)).map(|(pp, _)| pp).collect();
    let writes = sig.params.iter().any(|t| matches!(t, PTy::A));
    // Only in a loop whose body calls nothing but itself: the lengths
    // take registers, which a call-free loop body has to spare; a loop
    // with inlined callees is register-bound and reloading is cheaper.
    // (a call to a small call-free function that always inlines does not
    // count: after inlining the body is still call-free)
    let leaf_loop = calls_fn(&f.body, fid) && !calls_other_real(m, &f.body, fid);
    let looping = lp && leaf_loop && arr_ps.len() >= 2 && writes;
    sem.use_lens = looping;
    for pp in arr_ps.iter().filter(|_| looping) {
        sem.lens.insert(vn(*pp as u32), format!("l_v{pp}"));
    }
    let mut bb = Vec::new();
    sem.tail(&f.body, fid, lp, &mut bb);
    // one fuel unit per call and loop iteration, as in the dive form: native
    // code never suspends, but its work counts toward the enclosing dive's
    // budget (so parallel granularity tracks work, not dive calls). Counted
    // in a local and settled at each return, so it stays in a register.
    let mut body: Vec<S> =
        arr_ps.iter().filter(|_| looping).map(|pp| let_(format!("l_v{pp}"), Ty::Usize, p("arr_len_of", vec![as_u(v(vn(*pp as u32)))]))).collect();
    // the device's stack guard where native frames can pile up (no budget)
    if (calls_fn(&f.body, fid) && !lp) || recursive_via_others(m, fid) {
        body.push(do_(p("stack_guard", vec![])));
    }
    body.push(let_("fl", Ty::I64, i64_(1)));
    if lp {
        // a native loop never suspends and checks nothing per iteration: a
        // check in a hot loop blocks the backend's unrolling of short
        // counted loops (raytrace's device kernel 68 -> 385 ms). A runaway
        // native loop is stopped with its process (design section 7.6).
        let mut lb = vec![set("fl", bin(Bop::Add, v("fl"), i64_(1)))];
        lb.extend(bb);
        body.push(S::Loop(lb));
    } else {
        body.extend(bb);
    }
    let inl = if crate::inline_attr_fn(m, fid) { Inline::Always } else { Inline::Default };
    let native = FnDef { name: format!("s_{fid}"), ctx: !ctx_arg(fid).is_empty(), params, ret: ret_ty, body, inline: inl, cold: false };
    if !bridge {
        return vec![native];
    }

    let mut bridge_body = port_bridge(fid, &sig, &bor[fid as usize], Ty::Res);
    // a proven fold measures its work, then splits a large range before running
    // a chunk natively
    let split = crate::seq::FOLD_BRIDGE.with(|f| f.borrow().get(fid as usize).cloned().flatten());
    if let Some(mut pre) = split {
        pre.append(&mut bridge_body);
        bridge_body = pre;
    }
    // Every caller native: nothing dives this function, so the bridge
    // has no caller and must not look like one (a live bridge call site
    // with unknown arguments blocks the backend's interprocedural
    // constant propagation and single-call-site inlining).
    if !flag(&BRIDGE_LIVE, fid, true) {
        bridge_body = vec![S::Unreachable];
    }
    let mut dparams = vec![("fuel".to_string(), Ty::RefI64)];
    dparams.extend(vparams(ar));
    vec![native, FnDef { name: format!("d_{fid}"), ctx: true, params: dparams, ret: Ty::Res, body: bridge_body, inline: Inline::Default, cold: false }]
}
