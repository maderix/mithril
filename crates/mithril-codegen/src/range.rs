//! Value ranges of int expressions, for eliding the i56 wrap.
//!
//! (`&`, `|`, `^` of i56 values never need it: sign extension is preserved
//! bitwise.)
//!
//! Ints are i56 with wrapping arithmetic: raw i64 code re-wraps after every
//! operation (`wrap56`, a shift pair) unless the result provably stays in
//! i56. Ranges come from constants, masks (`x & c`), comparisons, shifts by
//! constants, division and modulo by positive constants, and let-bound
//! intermediates; parameters and call results are unknown (full i56). A
//! result that only feeds `& c` with `0 <= c < 2^55` needs no wrap either:
//! the wrap changes bits >= 56 only.

use mithril_front::ast::BinOp;
use mithril_front::core::Core;
use std::collections::{HashMap, HashSet};

type Iv = (i128, i128);
const MIN: i128 = -(1 << 55);
const MAX: i128 = (1 << 55) - 1;
const FULL: Iv = (MIN, MAX);

fn fits(v: Iv) -> bool {
    v.0 >= MIN && v.1 <= MAX
}

fn low_mask(e: &Core) -> bool {
    matches!(e, Core::Num(c) if (0..=MAX as i64).contains(c))
}

pub(crate) struct Ranges {
    vars: HashMap<u32, Iv>,
    /// let-bound vars every use of which is `var & c` (small nonneg c)
    masked: HashSet<u32>,
    /// ... with `c < 2^32`
    masked32: HashSet<u32>,
}

impl Ranges {
    pub(crate) fn of(body: &Core) -> Ranges {
        let mut r = Ranges { vars: HashMap::new(), masked: HashSet::new(), masked32: HashSet::new() };
        r.collect(body);
        let mut all = HashMap::new();
        let mut under_mask = HashMap::new();
        let mut under_mask32 = HashMap::new();
        count(body, &mut all, &mut under_mask, &mut under_mask32);
        for (v, n) in all {
            if n > 0 && under_mask.get(&v) == Some(&n) {
                r.masked.insert(v);
            }
            if n > 0 && under_mask32.get(&v) == Some(&n) {
                r.masked32.insert(v);
            }
        }
        r
    }

    fn collect(&mut self, e: &Core) {
        match e {
            Core::Let(x, rhs, b) => {
                self.collect(rhs);
                let iv = self.iv(rhs);
                self.vars.insert(*x, iv);
                self.collect(b);
            }
            Core::Op2(_, a, b) | Core::Cmp(_, a, b) => {
                self.collect(a);
                self.collect(b);
            }
            Core::If(c, t, f) => {
                self.collect(c);
                self.collect(t);
                self.collect(f);
            }
            Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
                xs.iter().for_each(|x| self.collect(x))
            }
            Core::Match(s, arms) => {
                self.collect(s);
                arms.iter().for_each(|(_, _, b)| self.collect(b));
            }
            Core::Proj(a, _) => self.collect(a),
            Core::Num(_) | Core::Flo(_) | Core::Var(_) => {}
        }
    }

    /// Range of an int expression (full i56 when unknown).
    pub(crate) fn iv(&self, e: &Core) -> Iv {
        match e {
            Core::Num(n) => (*n as i128, *n as i128),
            Core::Var(i) => self.vars.get(i).copied().unwrap_or(FULL),
            Core::Cmp(..) => (0, 1),
            Core::Op2(op, a, b) => {
                let raw = op_iv(op, self.iv(a), self.iv(b), b);
                if fits(raw) {
                    raw
                } else {
                    FULL
                }
            }
            Core::If(_, t, f) => {
                let (x, y) = (self.iv(t), self.iv(f));
                (x.0.min(y.0), x.1.max(y.1))
            }
            Core::Let(_, _, b) => self.iv(b),
            _ => FULL,
        }
    }

    /// Whether `Op2(op, a, b)` needs the i56 wrap; `low` = only its bits
    /// below 55 are observed (it feeds a small nonneg mask).
    pub(crate) fn wrap(&self, op: &BinOp, a: &Core, b: &Core, low: bool) -> bool {
        !low && !fits(op_iv(op, self.iv(a), self.iv(b), b))
    }

    /// The let-bound var `x` is only observed through small masks.
    pub(crate) fn masked(&self, x: u32) -> bool {
        self.masked.contains(&x)
    }

    /// ... through masks below 2^32.
    pub(crate) fn masked32(&self, x: u32) -> bool {
        self.masked32.contains(&x)
    }
}

/// `low` flag for the left operand of `Op2(op, _, b)`.
pub(crate) fn feeds_mask(op: &BinOp, b: &Core) -> bool {
    *op == BinOp::BitAnd && low_mask(b)
}

/// The left operand of `Op2(op, _, b)` is only observed in its low 32 bits.
pub(crate) fn feeds_mask32(op: &BinOp, b: &Core) -> bool {
    *op == BinOp::BitAnd && matches!(b, Core::Num(c) if (0..=u32::MAX as i64).contains(c))
}

/// Ops whose low 32 result bits depend only on the low 32 bits of their
/// operands (for `Shl`: of its left operand).
pub(crate) fn low32_closed(op: &BinOp) -> bool {
    matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Shl | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor)
}

fn op_iv(op: &BinOp, x: Iv, y: Iv, yexpr: &Core) -> Iv {
    let k = match yexpr {
        Core::Num(n) => Some(*n as i128),
        _ => None,
    };
    let prod = |a: i128, b: i128| a.saturating_mul(b);
    match op {
        BinOp::Add => (x.0 + y.0, x.1 + y.1),
        BinOp::Sub => (x.0 - y.1, x.1 - y.0),
        BinOp::Mul => {
            let ps = [prod(x.0, y.0), prod(x.0, y.1), prod(x.1, y.0), prod(x.1, y.1)];
            (*ps.iter().min().unwrap(), *ps.iter().max().unwrap())
        }
        BinOp::BitAnd => {
            if x.0 >= 0 && y.0 >= 0 {
                (0, x.1.min(y.1))
            } else if y.0 >= 0 {
                (0, y.1)
            } else if x.0 >= 0 {
                (0, x.1)
            } else {
                FULL
            }
        }
        BinOp::BitOr | BinOp::BitXor => {
            if x.0 >= 0 && y.0 >= 0 {
                let m = x.1.max(y.1);
                let bits = 128 - m.leading_zeros();
                (0, (1i128 << bits) - 1)
            } else {
                FULL // bitwise ops of i56 values are i56 values
            }
        }
        BinOp::Shl => match k {
            Some(s) if (0..=62).contains(&s) => (x.0 << s, x.1 << s),
            _ => (MIN * 2, MAX * 2),
        },
        BinOp::Shr => match k {
            Some(s) if (0..=62).contains(&s) => (x.0 >> s, x.1 >> s),
            _ => FULL,
        },
        BinOp::FloorDiv => match k {
            Some(d) if d > 0 => (x.0.div_euclid(d), x.1.div_euclid(d)),
            _ => (MIN * 2, MAX * 2),
        },
        BinOp::Div => match k {
            Some(d) if d > 0 => (x.0 / d, x.1 / d),
            _ => (MIN * 2, MAX * 2),
        },
        BinOp::Mod => match k {
            Some(d) if d > 0 => (0, d - 1),
            _ => (MIN * 2, MAX * 2),
        },
    }
}

/// All uses of each var, and uses of the form `var & small_mask`.
fn count(e: &Core, all: &mut HashMap<u32, usize>, masked: &mut HashMap<u32, usize>, m32: &mut HashMap<u32, usize>) {
    match e {
        Core::Var(i) => *all.entry(*i).or_insert(0) += 1,
        Core::Op2(op, a, b) => {
            if let Core::Var(i) = &**a {
                if feeds_mask(op, b) {
                    *masked.entry(*i).or_insert(0) += 1;
                }
                if feeds_mask32(op, b) {
                    *m32.entry(*i).or_insert(0) += 1;
                }
            }
            count(a, all, masked, m32);
            count(b, all, masked, m32);
        }
        Core::Cmp(_, a, b) => {
            count(a, all, masked, m32);
            count(b, all, masked, m32);
        }
        Core::If(c, t, f) => {
            count(c, all, masked, m32);
            count(t, all, masked, m32);
            count(f, all, masked, m32);
        }
        Core::Let(_, r, b) => {
            count(r, all, masked, m32);
            count(b, all, masked, m32);
        }
        Core::Call(_, xs) | Core::Ctor(_, xs) | Core::Tuple(xs) | Core::Reuse(_, _, xs) | Core::Prim(_, xs) => {
            xs.iter().for_each(|x| count(x, all, masked, m32))
        }
        Core::Match(s, arms) => {
            count(s, all, masked, m32);
            arms.iter().for_each(|(_, _, b)| count(b, all, masked, m32));
        }
        Core::Proj(a, _) => count(a, all, masked, m32),
        Core::Num(_) | Core::Flo(_) => {}
    }
}
