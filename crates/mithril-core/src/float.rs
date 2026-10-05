//! Float arithmetic: the one definition the oracle, the compile-time reducer
//! and the CPU runtime share. The device engines mirror it, and conformance
//! tests check every backend against it bit for bit.
//!
//! Every operation is IEEE 754 with round to nearest even, and no operation
//! fuses with another. NaN is the one place IEEE leaves the bits open: x86
//! produces a NaN with its sign set, arm64 and Apple GPUs one with its sign
//! clear, NVIDIA GPUs yet another, and some keep an operand's payload. So the
//! bits of a NaN are fixed wherever they can be observed: the canonical quiet
//! NaN with the sign clear (`NAN32`, `NAN64`).
//!
//! f32 arithmetic itself leaves NaN bits as the hardware makes them: no
//! operation tells one NaN from another, so they differ only where a value's
//! bits leave float arithmetic, and there the front end applies `f32_canon`
//! (the program's result; in strict mode every operation). f64 values are
//! boxed and leave only through the output, so `f64_op` canonicalizes at once.
//!
//! An f32 value travels as its bit pattern in an `i64` (low 32 bits), an
//! f16 value likewise (low 16 bits).
//!
//! f16 arithmetic is f32 arithmetic on the widened operands, rounded once to
//! f16: binary32 carries more than twice binary16's precision plus two bits,
//! so add, sub, mul, div and sqrt rounded through it are the correctly
//! rounded binary16 results. The two conversions below are all a backend
//! adds for f16, in integer arithmetic so every device computes them alike.
//! Every f16 result passes through `f32_to_f16`, which makes a NaN canonical
//! (`NAN16`), so f16 values are always canonical.

/// The canonical binary32 NaN.
pub const NAN32: u32 = 0x7fc0_0000;
/// The canonical binary64 NaN.
pub const NAN64: u64 = 0x7ff8_0000_0000_0000;
/// The canonical binary16 NaN.
pub const NAN16: u16 = 0x7e00;

#[inline(always)]
pub fn canon32(x: f32) -> f32 {
    if x.is_nan() { f32::from_bits(NAN32) } else { x }
}

#[inline(always)]
pub fn canon64(x: f64) -> f64 {
    if x.is_nan() { f64::from_bits(NAN64) } else { x }
}

#[inline(always)]
fn f(x: i64) -> f32 {
    f32::from_bits(x as u32)
}

#[inline(always)]
fn b(x: f32) -> i64 {
    x.to_bits() as i64
}

/// The bits of an f32 with a NaN made canonical: applied where an f32's bits
/// can be observed.
#[inline(always)]
pub fn f32_canon(x: i64) -> i64 {
    canon32(f(x)).to_bits() as i64
}

#[inline(always)]
pub fn f32_add(x: i64, y: i64) -> i64 {
    b(f(x) + f(y))
}

#[inline(always)]
pub fn f32_sub(x: i64, y: i64) -> i64 {
    b(f(x) - f(y))
}

#[inline(always)]
pub fn f32_mul(x: i64, y: i64) -> i64 {
    b(f(x) * f(y))
}

#[inline(always)]
pub fn f32_div(x: i64, y: i64) -> i64 {
    b(f(x) / f(y))
}

#[inline(always)]
pub fn f32_sqrt(x: i64) -> i64 {
    b(f(x).sqrt())
}

#[inline(always)]
pub fn f32_lt(x: i64, y: i64) -> i64 {
    (f(x) < f(y)) as i64
}

#[inline(always)]
pub fn f32_le(x: i64, y: i64) -> i64 {
    (f(x) <= f(y)) as i64
}

#[inline(always)]
pub fn f32_from_u32(x: i64) -> i64 {
    b((x as u32) as f32)
}

/// Truncation toward zero; NaN, negative and out-of-range values give 0.
#[inline(always)]
pub fn f32_to_u32(x: i64) -> i64 {
    let v = f(x);
    if v.is_nan() || v < 0.0 || v >= 4294967296.0 { 0 } else { v as u32 as i64 }
}

/// The f32 holding f16 `h` exactly.
#[inline]
pub fn f16_to_f32(h: i64) -> i64 {
    let h = h as u32 & 0xffff;
    let sign = (h & 0x8000) << 16;
    let exp = (h >> 10) & 0x1f;
    let man = h & 0x3ff;
    let bits = if exp == 0x1f {
        sign | 0x7f80_0000 | (man << 13)
    } else if exp != 0 {
        sign | ((exp + 112) << 23) | (man << 13)
    } else if man == 0 {
        sign
    } else {
        // subnormal: man * 2^-24, renormalized
        let p = 31 - man.leading_zeros();
        sign | ((p + 103) << 23) | ((man << (23 - p)) & 0x7f_ffff)
    };
    bits as i64
}

/// The f16 nearest f32 `x` (ties to even); a NaN gives the canonical NaN.
#[inline]
pub fn f32_to_f16(x: i64) -> i64 {
    let f = x as u32;
    let sign = (f >> 16) & 0x8000;
    let a = f & 0x7fff_ffff;
    let h = if a > 0x7f80_0000 {
        return NAN16 as i64;
    } else if a >= 0x477f_f000 {
        // from halfway between 65504 and 65536 up: infinity
        sign | 0x7c00
    } else if a >= 0x3880_0000 {
        // normal: rebias the exponent, round the mantissa to 10 bits
        let r = a + 0xfff + ((a >> 13) & 1);
        sign | ((r - 0x3800_0000) >> 13)
    } else if a <= 0x3300_0000 {
        // up to half the smallest subnormal (a tie rounds to even, zero)
        sign
    } else {
        // subnormal: m * 2^(e - 150) in units of 2^-24
        let e = a >> 23;
        let m = (a & 0x7f_ffff) | 0x80_0000;
        let shift = 126 - e;
        let (q, rem, half) = (m >> shift, m & ((1 << shift) - 1), 1 << (shift - 1));
        sign | (q + (rem > half || (rem == half && q & 1 == 1)) as u32)
    };
    h as i64
}

/// The f16 nearest `v` (ties to even): an f16 literal, rounded once from
/// its decimal value.
pub fn f64_to_f16(v: f64) -> i64 {
    if v.is_nan() {
        return NAN16 as i64;
    }
    let sign = if v.is_sign_negative() { 0x8000 } else { 0 };
    let a = v.abs();
    // the non-negative f16 patterns are ordered like their values: find the
    // last one at or below `a`, then pick the nearer neighbour
    // past the largest finite (0x7bff) the next step is 2^16, as if the
    // exponent went on: from halfway (65520) up a value rounds to infinity
    let val = |h: u32| match h {
        0x7c00 => 65536.0,
        _ => f32::from_bits(f16_to_f32(h as i64) as u32) as f64,
    };
    let (mut lo, mut hi) = (0u32, 0x7c00u32);
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if val(mid) <= a { lo = mid } else { hi = mid - 1 }
    }
    let h = if lo == 0x7c00 || val(lo) == a {
        lo
    } else {
        let (d0, d1) = (a - val(lo), val(lo + 1) - a);
        if d0 < d1 || (d0 == d1 && lo & 1 == 0) { lo } else { lo + 1 }
    };
    (sign | h) as i64
}

/// A binary f64 operation by opcode: 0 add, 1 sub, 2 mul, 3 div. `None` for
/// any other opcode.
#[inline(always)]
pub fn f64_op(op: u8, x: f64, y: f64) -> Option<f64> {
    Some(canon64(match op {
        0 => x + y,
        1 => x - y,
        2 => x * y,
        3 => x / y,
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO: i64 = 0;
    const ONE: i64 = 0x3f80_0000;
    const INF: i64 = 0x7f80_0000;
    const NEG_ONE: i64 = 0xbf80_0000;

    #[test]
    fn every_nan_an_f32_operation_makes_is_canonical_once_observed() {
        let nan = NAN32 as i64;
        let c = f32_canon;
        assert_eq!(c(f32_div(ZERO, ZERO)), nan, "0/0");
        assert_eq!(c(f32_sub(INF, INF)), nan, "inf-inf");
        assert_eq!(c(f32_mul(INF, ZERO)), nan, "inf*0");
        assert_eq!(c(f32_sqrt(NEG_ONE)), nan, "sqrt(-1)");
        // a NaN operand with a payload and the sign set
        let odd = 0xffc0_1234u32 as i64;
        assert_eq!(c(f32_add(odd, ONE)), nan);
        assert_eq!(c(f32_mul(ONE, odd)), nan);
        assert_eq!(c(odd), nan);
        // every non-NaN pattern passes through unchanged
        for x in [ZERO, ONE, INF, NEG_ONE, 0x8000_0000, 0x0000_0001, 0xff80_0000] {
            assert_eq!(c(x), x);
        }
        // comparisons with any NaN are false; conversion of a NaN gives 0
        for n in [nan, odd] {
            assert_eq!((f32_lt(n, ONE), f32_le(ONE, n), f32_to_u32(n)), (0, 0, 0));
        }
    }

    #[test]
    fn every_nan_an_f64_operation_makes_is_canonical() {
        let odd = f64::from_bits(0xfff8_0000_0000_1234);
        for (op, x, y) in [(3, 0.0, 0.0), (1, f64::INFINITY, f64::INFINITY), (2, f64::INFINITY, 0.0), (0, odd, 1.0), (2, 1.0, odd)] {
            assert_eq!(f64_op(op, x, y).unwrap().to_bits(), NAN64, "op {op}");
        }
        assert_eq!(f64_op(4, 1.0, 1.0), None);
    }

    #[test]
    fn every_f16_widens_exactly_and_narrows_back() {
        for h in 0..=0xffffu32 {
            let w = f16_to_f32(h as i64) as u32;
            let back = f32_to_f16(w as i64) as u32;
            let nan = (h & 0x7c00) == 0x7c00 && h & 0x3ff != 0;
            if nan {
                assert!(f32::from_bits(w).is_nan());
                assert_eq!(back, NAN16 as u32, "NaN {h:#06x}");
            } else {
                assert_eq!(back, h, "{h:#06x} -> {w:#010x} -> {back:#06x}");
                // the widened value is the f16's value
                assert_eq!(f64_to_f16(f32::from_bits(w) as f64) as u32, h, "{h:#06x}");
            }
        }
    }

    #[test]
    fn f32_to_f16_rounds_to_nearest_even_like_an_exact_search() {
        let mut s = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        // random patterns, and every f16 boundary with its neighbours: halfway
        // points, the top of the normal range, the subnormal range and zero
        let mut cases: Vec<u32> = (0..200_000).map(|_| next() as u32).collect();
        for h in 0..0x7c00u32 {
            let w = f16_to_f32(h as i64) as u32;
            let up = f16_to_f32(h as i64 + 1) as u32;
            let mid = ((f32::from_bits(w) as f64 + f32::from_bits(up) as f64) / 2.0) as f32;
            for b in [w, mid.to_bits()] {
                cases.extend([b.wrapping_sub(1), b, b + 1, b | 0x8000_0000]);
            }
        }
        cases.extend([0x477f_efff, 0x477f_f000, 0x477f_f001, 0x3300_0000, 0x3300_0001, 0x7f80_0000, 0xff80_0000]);
        for x in cases {
            let f = f32::from_bits(x);
            let got = f32_to_f16(x as i64);
            assert_eq!(got, f64_to_f16(f as f64), "{x:#010x} ({f:e})");
        }
    }

    #[test]
    fn f16_arithmetic_through_f32_is_the_correctly_rounded_result() {
        // add, mul, div of f16 values through f32, against the exact f64
        // result rounded once to f16
        let mut s = 0x1234_5678_9abc_def1u64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let w = |h: u32| f16_to_f32(h as i64);
        let v = |h: u32| f32::from_bits(w(h) as u32) as f64;
        for _ in 0..200_000 {
            let (a, b) = ((next() as u32) & 0xffff, (next() as u32) & 0xffff);
            if [a, b].iter().any(|&h| (h & 0x7c00) == 0x7c00) {
                continue;
            }
            for (got, exact) in [(f32_add(w(a), w(b)), v(a) + v(b)), (f32_mul(w(a), w(b)), v(a) * v(b)), (f32_div(w(a), w(b)), v(a) / v(b)), (f32_sqrt(w(a)), v(a).sqrt())] {
                let narrowed = f32_to_f16(got);
                let want = f64_to_f16(exact);
                if (want & 0x7c00) == 0x7c00 && want & 0x3ff != 0 {
                    assert_eq!(narrowed, NAN16 as i64);
                } else {
                    assert_eq!(narrowed, want, "{a:#06x} {b:#06x}");
                }
            }
        }
    }

    #[test]
    fn non_nan_results_are_the_hardware_ieee_results() {
        let mut s = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..100_000 {
            let (r, t) = (next(), next());
            let (x, y) = ((r as u32) as i64, (t as u32) as i64);
            let (fx, fy) = (f32::from_bits(x as u32), f32::from_bits(y as u32));
            for (got, want) in [(f32_add(x, y), fx + fy), (f32_sub(x, y), fx - fy), (f32_mul(x, y), fx * fy), (f32_div(x, y), fx / fy), (f32_sqrt(x), fx.sqrt())] {
                if want.is_nan() {
                    assert_eq!(f32_canon(got), NAN32 as i64);
                } else {
                    assert_eq!(got, want.to_bits() as i64);
                }
            }
            let (dx, dy) = (f64::from_bits(r), f64::from_bits(t));
            for (op, want) in [(0u8, dx + dy), (1, dx - dy), (2, dx * dy), (3, dx / dy)] {
                let got = f64_op(op, dx, dy).unwrap().to_bits();
                assert_eq!(got, if want.is_nan() { NAN64 } else { want.to_bits() });
            }
        }
    }
}
