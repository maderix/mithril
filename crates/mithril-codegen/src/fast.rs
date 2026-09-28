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

use crate::seq::{bin_code, cmp_code};
use crate::ty::{Ty, Types};
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
}

impl Fp<'_> {
    fn arity(&self) -> usize {
        self.m.fns[self.fid as usize].arity
    }

    fn is_param(&self, v: u32) -> bool {
        (v as usize) < self.arity()
    }

    fn int_param(&self, v: u32) -> bool {
        self.is_param(v) && self.tys.var(self.fid as usize, v) == Ty::Int
    }

    fn fallback(&mut self, b: &mut String) {
        self.falls += 1;
        let args: String = (0..self.arity()).map(|i| format!(", v{i}")).collect();
        b.push_str(&format!("return d_{}(ctx, fuel{args});\n", self.fid));
    }

    /// A pure int expression as raw i64 Rust, or None.
    fn pexpr(&self, e: &Core) -> Option<String> {
        self.pexpr_low(e, false)
    }

    fn pexpr_low(&self, e: &Core, low: bool) -> Option<String> {
        Some(match e {
            Core::Num(n) => format!("{n}i64"),
            Core::Var(i) if self.ints.contains(i) => format!("w{i}"),
            Core::Var(i) if self.int_param(*i) => format!("as_i(v{i})"),
            Core::Op2(op, a, b) => {
                let (x, y) = (self.pexpr_low(a, crate::range::feeds_mask(op, b))?, self.pexpr(b)?);
                let body = match bin_code(op) {
                    0 => format!("{x}.wrapping_add({y})"),
                    1 => format!("{x}.wrapping_sub({y})"),
                    2 => format!("{x}.wrapping_mul({y})"),
                    3 => format!("{x}.wrapping_div({y})"),
                    4 => format!("floor_div({x}, {y})"),
                    5 => format!("py_mod({x}, {y})"),
                    6 => format!("{x}.wrapping_shl({y} as u32)"),
                    7 => format!("{x}.wrapping_shr({y} as u32)"),
                    8 => format!("({x} & {y})"),
                    9 => format!("({x} | {y})"),
                    _ => format!("({x} ^ {y})"),
                };
                if self.ranges.wrap(op, a, b, low) {
                    format!("wrap56({body})")
                } else {
                    body
                }
            }
            Core::Cmp(op, a, b) => {
                let (x, y) = (self.pexpr(a)?, self.pexpr(b)?);
                let o = ["<", "<=", ">", ">=", "==", "!="][cmp_code(op) as usize];
                format!("(({x} {o} {y}) as i64)")
            }
            Core::If(c, t, f) => {
                format!("(if {} != 0 {{ {} }} else {{ {} }})", self.pexpr(c)?, self.pexpr(t)?, self.pexpr(f)?)
            }
            _ => return None,
        })
    }

    /// Release owned boxed params this committed path does not return.
    fn frees(&self, keep: Option<u32>, b: &mut String) {
        for p in 0..self.arity() as u32 {
            if Some(p) != keep && !self.int_param(p) && !self.bor[p as usize] && !self.imm.contains(&p) {
                b.push_str(&format!("free_val(ctx, v{p});\n"));
            }
        }
    }

    fn tail(&mut self, e: &Core, b: &mut String) {
        match e {
            Core::Let(x, r, bo) => match self.pexpr(r) {
                Some(_) => {
                    let s = self.pexpr_low(r, self.ranges.masked(*x)).unwrap();
                    b.push_str(&format!("let w{x}: i64 = {s};\n"));
                    self.ints.insert(*x);
                    self.tail(bo, b);
                }
                None => self.fallback(b),
            },
            Core::If(c, t, f) => match self.pexpr(c) {
                Some(s) => {
                    b.push_str(&format!("if {s} != 0 {{\n"));
                    let saved = (self.ints.clone(), self.imm.clone());
                    self.tail(t, b);
                    (self.ints, self.imm) = saved.clone();
                    b.push_str("} else {\n");
                    self.tail(f, b);
                    (self.ints, self.imm) = saved;
                    b.push_str("}\n");
                }
                None => self.fallback(b),
            },
            Core::Match(s, arms) => {
                let Core::Var(v) = **s else { return self.fallback(b) };
                if !self.is_param(v) || self.int_param(v) {
                    return self.fallback(b);
                }
                let mut first = true;
                for (c, binders, body) in arms {
                    let cond = if let Some(slot) = self.unbox.get(c) {
                        format!("tag(v{v}) == TU + {slot}")
                    } else if self.m.ctors.get(*c as usize).is_some_and(|x| x.1 == 0) {
                        format!("v{v} == con(0, {c}u16, 0)")
                    } else {
                        continue; // a boxed arm: its cell would be consumed
                    };
                    b.push_str(&format!("{}if {cond} {{\n", if first { "" } else { "} else " }));
                    first = false;
                    let saved = (self.ints.clone(), self.imm.clone());
                    self.imm.push(v);
                    if let (Some(_), Some(bv)) = (self.unbox.get(c), binders.first()) {
                        b.push_str(&format!("let w{bv}: i64 = as_i(v{v});\n"));
                        self.ints.insert(*bv);
                    }
                    self.tail(body, b);
                    (self.ints, self.imm) = saved;
                }
                if first {
                    return self.fallback(b);
                }
                b.push_str("} else {\n");
                self.fallback(b);
                b.push_str("}\n");
            }
            Core::Ctor(c, args) | Core::Reuse(_, c, args) => {
                if let (Some(slot), [a]) = (self.unbox.get(c), args.as_slice()) {
                    if let Some(s) = self.pexpr(a) {
                        self.frees(None, b);
                        self.tails += 1;
                        return b.push_str(&format!("return Ok(ic({slot}u64, {s}));\n"));
                    }
                } else if args.is_empty() {
                    self.frees(None, b);
                    self.tails += 1;
                    return b.push_str(&format!("return Ok(con(0, {c}u16, 0));\n"));
                }
                self.fallback(b)
            }
            Core::Var(p) if self.is_param(*p) && !self.int_param(*p) => {
                self.frees(Some(*p), b);
                self.tails += 1;
                if self.bor[*p as usize] {
                    b.push_str(&format!("return Ok(dup_val(ctx, v{p}));\n"));
                } else {
                    b.push_str(&format!("return Ok(v{p});\n"));
                }
            }
            other => match self.pexpr(other) {
                Some(s) => {
                    self.frees(None, b);
                    self.tails += 1;
                    b.push_str(&format!("return Ok(num({s}));\n"));
                }
                None => self.fallback(b),
            },
        }
    }
}

fn calls_self(fid: u32, e: &Core) -> bool {
    match e {
        Core::Num(_) | Core::Flo(_) | Core::Var(_) => false,
        Core::Call(g, xs) => *g == fid || xs.iter().any(|x| calls_self(fid, x)),
        Core::Op2(_, a, b) | Core::Cmp(_, a, b) => calls_self(fid, a) || calls_self(fid, b),
        Core::If(a, b, c) => calls_self(fid, a) || calls_self(fid, b) || calls_self(fid, c),
        Core::Let(_, r, b) => calls_self(fid, r) || calls_self(fid, b),
        Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => xs.iter().any(|x| calls_self(fid, x)),
        Core::Match(s, arms) => calls_self(fid, s) || arms.iter().any(|(_, _, b)| calls_self(fid, b)),
        Core::Proj(a, _) => calls_self(fid, a),
    }
}

/// The `q_<fid>` wrapper, when `fid` is self-recursive and has both an
/// inlinable base case and a fallback.
pub(crate) fn fast_fn(
    m: &CoreModule,
    fid: u32,
    body: &Core,
    tys: &Types,
    unbox: &HashMap<u32, u8>,
    bor: &[bool],
) -> Option<String> {
    if !calls_self(fid, body) {
        return None;
    }
    let ranges = crate::range::Ranges::of(body);
    let mut fp = Fp { m, fid, tys, unbox, bor, ints: HashSet::new(), ranges, imm: Vec::new(), tails: 0, falls: 0 };
    let mut b = String::new();
    fp.tail(body, &mut b);
    if fp.tails == 0 || fp.falls == 0 {
        return None;
    }
    let ar = m.fns[fid as usize].arity;
    let params: String = (0..ar).map(|i| format!(", v{i}: u64")).collect();
    Some(format!(
        "#[inline(always)]\n#[allow(clippy::too_many_arguments)]\nfn q_{fid}(ctx: &mut Wctx, fuel: &mut i64{params}) -> R {{\n{b}unreachable!()\n}}\n\n"
    ))
}
