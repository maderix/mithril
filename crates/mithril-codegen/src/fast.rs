//! Partial inlining of base cases.
//!
//! A recursive function over trees is mostly called on leaves, and a call
//! costs a frame, a fuel check and a dispatch that the leaf arm itself does
//! not need. LLVM will not inline a recursive function into itself, so for
//! each self-recursive dive function `f` this derives `q_f` from `f`'s own
//! body: it follows matches on parameters whose arm constructors carry no
//! cell (unboxed unary ints, nullary ctors) and pure int code, returns the
//! arm's value inline when the arguments reach a call-free, allocation-free
//! tail, and otherwise calls `d_f` with the original arguments. A base case
//! that calls anything is not inlined: it is only worth it when the base
//! case is cheap next to a call (and force-inlining a heavy callee changes
//! how LLVM compiles it). Every
//! fallback happens before any side effect, so it is always valid; call
//! sites in dive forms call `q_f` instead of `d_f`.

use crate::lir::{as_i, bin, c, cast, free, i64_, let_, num, ok, p, ret, set, u16_, u32_, u64_, u8_, v, Bop, FnDef, Inline, Ty, E, S};
use crate::seq::{arith, bin_code, cmp_code, compare, vn, vparams};
use crate::ty::{Ty as CTy, Types};
use mithril_front::core::{Core, CoreModule};
use std::collections::{HashMap, HashSet};

thread_local! {
    /// `FAST[g]`: a `q_<g>` wrapper exists for the module being emitted.
    pub(crate) static FAST: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(crate) fn has_fast(g: u32) -> bool {
    FAST.with(|f| f.borrow().get(g as usize).copied().unwrap_or(false))
}

struct Fp<'m> {
    m: &'m CoreModule,
    fid: u32,
    tys: &'m Types,
    unbox: &'m HashMap<u32, u8>,
    bor: &'m [bool],
    /// int locals, held as raw i64 `w<i>`
    ints: HashSet<u32>,
    ranges: crate::range::Ranges,
    /// params whose value is a cell-free immediate on the current path
    imm: Vec<u32>,
    tails: usize,
    falls: usize,
    tmp: u32,
}

impl Fp<'_> {
    fn arity(&self) -> usize {
        self.m.fns[self.fid as usize].arity
    }

    fn is_param(&self, v: u32) -> bool {
        (v as usize) < self.arity()
    }

    fn int_param(&self, v: u32) -> bool {
        self.is_param(v) && self.tys.var(self.fid as usize, v) == CTy::Int
    }

    fn fallback(&mut self, b: &mut Vec<S>) {
        self.falls += 1;
        let mut args = vec![v("fuel")];
        args.extend((0..self.arity()).map(|i| v(vn(i as u32))));
        b.push(ret(E::Call { f: format!("d_{}", self.fid), ctx: true, args }));
    }

    /// Whether `e` is a pure int expression.
    fn pure(&self, e: &Core) -> bool {
        match e {
            Core::Num(_) => true,
            Core::Var(i) => self.ints.contains(i) || self.int_param(*i),
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => self.pure(a) && self.pure(b),
            Core::If(c, t, f) => self.pure(c) && self.pure(t) && self.pure(f),
            _ => false,
        }
    }

    /// A pure int expression as raw i64 (statements for value branches
    /// into `b`); `low` = only its low bits are observed.
    fn pexpr(&mut self, e: &Core, low: bool, b: &mut Vec<S>) -> E {
        match e {
            Core::Num(n) => i64_(*n),
            Core::Var(i) if self.ints.contains(i) => v(format!("w{i}")),
            Core::Var(i) => as_i(v(vn(*i))),
            Core::Op2(op, x, y) => {
                let (ex, ey) = (self.pexpr(x, crate::range::feeds_mask(op, y), b), self.pexpr(y, false, b));
                let body = arith(bin_code(op), ex, ey);
                if self.ranges.wrap(op, x, y, low) {
                    p("wrap56", vec![body])
                } else {
                    body
                }
            }
            Core::Cmp(op, x, y) => {
                let (ex, ey) = (self.pexpr(x, false, b), self.pexpr(y, false, b));
                cast(compare(cmp_code(op), ex, ey), Ty::I64)
            }
            Core::If(cd, t, f) => {
                let ec = self.pexpr(cd, false, b);
                self.tmp += 1;
                let tn = format!("x{}", self.tmp);
                let mut bt = Vec::new();
                let et = self.pexpr(t, false, &mut bt);
                bt.push(set(&tn, et));
                let mut bf = Vec::new();
                let ef = self.pexpr(f, false, &mut bf);
                bf.push(set(&tn, ef));
                b.push(S::Decl(tn.clone(), Ty::I64));
                b.push(S::If(bin(Bop::Ne, ec, i64_(0)), bt, bf));
                v(tn)
            }
            _ => unreachable!("fast: non-pure int expression"),
        }
    }

    /// Release owned boxed params this committed path does not return.
    fn frees(&self, keep: Option<u32>, b: &mut Vec<S>) {
        for pp in 0..self.arity() as u32 {
            if Some(pp) != keep && !self.int_param(pp) && !self.bor[pp as usize] && !self.imm.contains(&pp) {
                b.push(free(v(vn(pp))));
            }
        }
    }

    fn tail(&mut self, e: &Core, b: &mut Vec<S>) {
        match e {
            Core::Let(x, r, bo) if self.pure(r) => {
                let low = self.ranges.masked(*x);
                let s = self.pexpr(r, low, b);
                b.push(let_(format!("w{x}"), Ty::I64, s));
                self.ints.insert(*x);
                self.tail(bo, b);
            }
            Core::Let(..) => self.fallback(b),
            Core::If(cd, t, f) if self.pure(cd) => {
                let s = self.pexpr(cd, false, b);
                let saved = (self.ints.clone(), self.imm.clone());
                let mut bt = Vec::new();
                self.tail(t, &mut bt);
                (self.ints, self.imm) = saved.clone();
                let mut bf = Vec::new();
                self.tail(f, &mut bf);
                (self.ints, self.imm) = saved;
                b.push(S::If(bin(Bop::Ne, s, i64_(0)), bt, bf));
            }
            Core::If(..) => self.fallback(b),
            Core::Match(s, arms) => {
                let Core::Var(x) = **s else { return self.fallback(b) };
                if !self.is_param(x) || self.int_param(x) {
                    return self.fallback(b);
                }
                // an if/else chain over the cell-free arms; boxed arms (whose
                // cell would be consumed) fall back
                let mut chain: Vec<(E, Vec<S>)> = Vec::new();
                for (cid, binders, body) in arms {
                    let cond = if let Some(slot) = self.unbox.get(cid) {
                        bin(Bop::Eq, p("tag", vec![v(vn(x))]), bin(Bop::Add, E::Const("TU".into()), u64_(*slot as u64)))
                    } else if self.m.ctors.get(*cid as usize).is_some_and(|c| c.1 == 0) {
                        bin(Bop::Eq, v(vn(x)), p("con", vec![u32_(0), u16_(*cid as u64), u8_(0)]))
                    } else {
                        continue;
                    };
                    let saved = (self.ints.clone(), self.imm.clone());
                    self.imm.push(x);
                    let mut ab = Vec::new();
                    if let (Some(_), Some(bv)) = (self.unbox.get(cid), binders.first()) {
                        ab.push(let_(format!("w{bv}"), Ty::I64, as_i(v(vn(x)))));
                        self.ints.insert(*bv);
                    }
                    self.tail(body, &mut ab);
                    (self.ints, self.imm) = saved;
                    chain.push((cond, ab));
                }
                if chain.is_empty() {
                    return self.fallback(b);
                }
                let mut rest = Vec::new();
                self.fallback(&mut rest);
                for (cond, ab) in chain.into_iter().rev() {
                    rest = vec![S::If(cond, ab, rest)];
                }
                b.extend(rest);
            }
            Core::Ctor(cid, args) | Core::Reuse(_, cid, args) => {
                if let (Some(slot), [a]) = (self.unbox.get(cid), args.as_slice()) {
                    if self.pure(a) {
                        let s = self.pexpr(a, false, b);
                        self.frees(None, b);
                        self.tails += 1;
                        return b.push(ret(ok(p("ic", vec![u64_(*slot as u64), s]))));
                    }
                } else if args.is_empty() {
                    self.frees(None, b);
                    self.tails += 1;
                    return b.push(ret(ok(p("con", vec![u32_(0), u16_(*cid as u64), u8_(0)]))));
                }
                self.fallback(b)
            }
            Core::Var(pp) if self.is_param(*pp) && !self.int_param(*pp) => {
                self.frees(Some(*pp), b);
                self.tails += 1;
                if self.bor[*pp as usize] {
                    b.push(ret(ok(c("dup_val", vec![v(vn(*pp))]))));
                } else {
                    b.push(ret(ok(v(vn(*pp)))));
                }
            }
            other if self.pure(other) => {
                let s = self.pexpr(other, false, b);
                self.frees(None, b);
                self.tails += 1;
                b.push(ret(ok(num(s))));
            }
            _ => self.fallback(b),
        }
    }
}

/// The `q_<fid>` wrapper, when `fid` is self-recursive and has both an
/// inlinable base case and a fallback.
pub(crate) fn fast_fn(m: &CoreModule, fid: u32, body: &Core, tys: &Types, unbox: &HashMap<u32, u8>, bor: &[bool]) -> Option<FnDef> {
    if !crate::scalar::calls_fn(body, fid) {
        return None;
    }
    let ranges = crate::range::Ranges::of(body);
    let mut fp = Fp { m, fid, tys, unbox, bor, ints: HashSet::new(), ranges, imm: Vec::new(), tails: 0, falls: 0, tmp: 0 };
    let mut b = Vec::new();
    fp.tail(body, &mut b);
    if fp.tails == 0 || fp.falls == 0 {
        return None;
    }
    b.push(S::Unreachable);
    let mut params = vec![("fuel".to_string(), Ty::RefI64)];
    params.extend(vparams(m.fns[fid as usize].arity));
    Some(FnDef { name: format!("q_{fid}"), ctx: true, params, ret: Ty::Res, body: b, inline: Inline::Always, cold: false })
}
