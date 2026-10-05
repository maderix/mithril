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
//! An f32 value travels as its bit pattern in an `i64` (low 32 bits).

/// The canonical binary32 NaN.
pub const NAN32: u32 = 0x7fc0_0000;
/// The canonical binary64 NaN.
pub const NAN64: u64 = 0x7ff8_0000_0000_0000;

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
