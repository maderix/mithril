//! Value ranges of int expressions.
//!
//! Ints are 64-bit. A value that provably fits 56 bits is *small*: it is an
//! immediate in a tagged word, so dive code copies it freely; any other int
//! may be a boxed `T_BIG` and is owned like any heap value. Ranges come from
//! constants, masks (`x & c`), comparisons, shifts by constants, division and
//! modulo by positive constants, and let-bound intermediates; parameters and
//! call results are unknown. The masked sets drive 32-bit native arithmetic.

use mithril_front::ast::BinOp;
use mithril_front::core::Core;
use std::collections::{HashMap, HashSet};

type Iv = (i128, i128);
/// The inline (immediate) range of a tagged int.
const MIN: i128 = -(1 << 55);
const MAX: i128 = (1 << 55) - 1;
/// Any 64-bit int.
const FULL: Iv = (i64::MIN as i128, i64::MAX as i128);
const OVER: Iv = FULL;

/// Within 64 bits (else the arithmetic wrapped: unknown).
fn fits(v: Iv) -> bool {
    v.0 >= FULL.0 && v.1 <= FULL.1
}

pub(crate) struct Ranges {
    vars: HashMap<u32, Iv>,
    /// let-bound vars every use of which is `var & c` with `0 <= c < 2^32`
    masked32: HashSet<u32>,
}

impl Ranges {
    pub(crate) fn of(body: &Core) -> Ranges {
        let mut r = Ranges { vars: HashMap::new(), masked32: HashSet::new() };
        r.collect(body);
        let mut all = HashMap::new();
        let mut under_mask32 = HashMap::new();
        count(body, &mut all, &mut under_mask32);
        for (v, n) in all {
            if n > 0 && under_mask32.get(&v) == Some(&n) {
                r.masked32.insert(v);
            }
        }
        r
    }

    fn collect(&mut self, e: &Core) {
        if let Core::Let(x, rhs, _) = e {
            self.collect(rhs);
            let iv = self.iv(rhs);
            self.vars.insert(*x, iv);
        }
        e.kids().into_iter().for_each(|k| self.collect(k));
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
            // an array's length fits its header's 48-bit field
            Core::Prim(mithril_front::core::Prim::ArrLen, _) => (0, (1 << 48) - 1),
            _ => FULL,
        }
    }

    /// The let-bound var `x` provably fits the inline 56-bit range.
    pub(crate) fn small_var(&self, x: u32) -> bool {
        self.vars.get(&x).is_some_and(|&(lo, hi)| lo >= MIN && hi <= MAX)
    }

    /// `e` provably fits the inline 56-bit range.
    pub(crate) fn small(&self, e: &Core) -> bool {
        let (lo, hi) = self.iv(e);
        lo >= MIN && hi <= MAX
    }

    /// The let-bound var `x` is only observed through masks below 2^32.
    pub(crate) fn masked32(&self, x: u32) -> bool {
        self.masked32.contains(&x)
    }
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

pub(crate) fn op_iv(op: &BinOp, x: Iv, y: Iv, yexpr: &Core) -> Iv {
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
            _ => OVER,
        },
        BinOp::Shr => match k {
            Some(s) if (0..=62).contains(&s) => (x.0 >> s, x.1 >> s),
            _ => FULL,
        },
        BinOp::Div | BinOp::FloorDiv | BinOp::Mod => match k {
            Some(d) if d > 0 => match op {
                BinOp::Mod => (0, d - 1),
                BinOp::Div => (x.0 / d, x.1 / d),
                _ => (x.0.div_euclid(d), x.1.div_euclid(d)),
            },
            _ => OVER,
        },
    }
}

/// All uses of each var, and uses of the form `var & mask32`.
fn count(e: &Core, all: &mut HashMap<u32, usize>, m32: &mut HashMap<u32, usize>) {
    e.walk(&mut |e| match e {
        Core::Var(i) => *all.entry(*i).or_insert(0) += 1,
        Core::Op2(op, a, b) => {
            if let Core::Var(i) = &**a {
                if feeds_mask32(op, b) {
                    *m32.entry(*i).or_insert(0) += 1;
                }
            }
        }
        _ => {}
    });
}
