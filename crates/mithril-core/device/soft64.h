// Software IEEE 754 binary64 in 64-bit integer arithmetic, for devices
// without double (Metal), and the exact binary32 path through it, for
// devices whose f32 flushes subnormals to zero.
//
// Every function takes and returns bit patterns. Results are correctly
// rounded to nearest even, and every NaN result is the canonical NaN
// (mithril_core::float). The same text compiles as C++ (tested on the host
// against hardware binary64: crates/mithril-core/tests/soft64_test.rs) and
// as MSL, so the code the test checks is the code the device runs.
//
// The add, subtract and multiply structure (significands with the leading
// bit at bit 62, the exponent one below the result's, round-and-pack by
// addition) follows Berkeley SoftFloat 3 (BSD-3-Clause, John R. Hauser).
// Division and square root are restoring digit loops: slower than
// SoftFloat's reciprocal estimates, and exact by construction.

#ifndef MITHRIL_SOFT64_H
#define MITHRIL_SOFT64_H

#ifdef __METAL_VERSION__
#include <metal_stdlib>
typedef ulong sf_u64;
typedef uint sf_u32;
typedef long sf_i64;
#define SF_THREAD thread
inline int sf_clz64(sf_u64 x) { return (int)metal::clz(x); }
#else
#include <stdint.h>
typedef uint64_t sf_u64;
typedef uint32_t sf_u32;
typedef int64_t sf_i64;
#define SF_THREAD
inline int sf_clz64(sf_u64 x) { return x ? __builtin_clzll(x) : 64; }
#endif

#define SF_NAN64 ((sf_u64)0x7ff8000000000000)
#define SF_NAN32 ((sf_u32)0x7fc00000)
#define SF_ONE ((sf_u64)1)

inline sf_i64 sf_exp(sf_u64 a) { return (sf_i64)((a >> 52) & 0x7ff); }
inline sf_u64 sf_sig(sf_u64 a) { return a & ((SF_ONE << 52) - 1); }

// Packing adds the fields, so a significand whose leading bit sits at bit
// 52 raises the exponent by one: callers pass the exponent one below.
inline sf_u64 sf_pack(sf_u64 sign, sf_i64 exp, sf_u64 sig) {
  return (sign << 63) + ((sf_u64)exp << 52) + sig;
}

// a >> dist with the bits shifted out ORed into bit 0 (the sticky bit).
inline sf_u64 sf_jam(sf_u64 a, sf_i64 dist) {
  if (dist == 0) return a;
  if (dist >= 63) return (sf_u64)(a != 0);
  return (a >> dist) | (sf_u64)((a << (64 - dist)) != 0);
}

// A subnormal significand shifted so its leading bit is bit 52; the
// exponent it then has.
inline sf_i64 sf_normalize(SF_THREAD sf_u64 *sig) {
  int s = sf_clz64(*sig) - 11;
  *sig <<= s;
  return 1 - s;
}

// Round `sig` (leading bit at 62, ten bits below the 53-bit significand)
// to nearest even and pack it; `exp` is one below the result's biased
// exponent. Handles subnormal results and overflow to infinity.
inline sf_u64 sf_round_pack(sf_u64 sign, sf_i64 exp, sf_u64 sig) {
  sf_u64 low = sig & 0x3ff;
  if ((sf_u64)exp >= 0x7fd) {
    if (exp < 0) {
      sig = sf_jam(sig, -exp);
      exp = 0;
      low = sig & 0x3ff;
    } else if (exp > 0x7fd || sig + 0x200 >= (SF_ONE << 63)) {
      return sf_pack(sign, 0x7ff, 0);
    }
  }
  sig = (sig + 0x200) >> 10;
  if (low == 0x200) sig &= ~SF_ONE;
  if (sig == 0) exp = 0;
  return sf_pack(sign, exp, sig);
}

// sf_round_pack for a significand whose leading bit may sit anywhere.
inline sf_u64 sf_norm_round_pack(sf_u64 sign, sf_i64 exp, sf_u64 sig) {
  int shift = sf_clz64(sig) - 1;
  exp -= shift;
  if (shift >= 10 && (sf_u64)exp < 0x7fd) return sf_pack(sign, sig ? exp : 0, sig << (shift - 10));
  return sf_round_pack(sign, exp, sig << shift);
}

// |a| + |b| with the given sign.
inline sf_u64 sf_add_mags(sf_u64 a, sf_u64 b, sf_u64 sign) {
  sf_i64 ea = sf_exp(a), eb = sf_exp(b);
  sf_u64 ma = sf_sig(a), mb = sf_sig(b);
  sf_i64 d = ea - eb;
  if (d == 0) {
    if (ea == 0) return sf_pack(sign, 0, ma + mb);
    if (ea == 0x7ff) return (ma | mb) ? SF_NAN64 : a;
    return sf_round_pack(sign, ea, ((SF_ONE << 53) + ma + mb) << 9);
  }
  ma <<= 9;
  mb <<= 9;
  sf_i64 ez;
  if (d < 0) {
    if (eb == 0x7ff) return mb ? SF_NAN64 : sf_pack(sign, 0x7ff, 0);
    ez = eb;
    ma = ea ? ma + (SF_ONE << 61) : ma << 1;
    ma = sf_jam(ma, -d);
  } else {
    if (ea == 0x7ff) return ma ? SF_NAN64 : a;
    ez = ea;
    mb = eb ? mb + (SF_ONE << 61) : mb << 1;
    mb = sf_jam(mb, d);
  }
  sf_u64 z = (SF_ONE << 61) + ma + mb;
  if (z < (SF_ONE << 62)) {
    ez--;
    z <<= 1;
  }
  return sf_round_pack(sign, ez, z);
}

// |a| - |b|, with a's sign flipped when |b| is larger.
inline sf_u64 sf_sub_mags(sf_u64 a, sf_u64 b, sf_u64 sign) {
  sf_i64 ea = sf_exp(a), eb = sf_exp(b);
  sf_u64 ma = sf_sig(a), mb = sf_sig(b);
  sf_i64 d = ea - eb;
  if (d == 0) {
    if (ea == 0x7ff) return SF_NAN64;  // inf - inf, or a NaN
    sf_i64 diff = (sf_i64)ma - (sf_i64)mb;
    if (diff == 0) return 0;  // x - x is +0 when rounding to nearest
    if (ea) ea--;
    if (diff < 0) {
      sign ^= 1;
      diff = -diff;
    }
    // exact: equal exponents cancel without a rounding
    sf_i64 shift = sf_clz64((sf_u64)diff) - 11;
    sf_i64 ez = ea - shift;
    if (ez < 0) {
      shift = ea;
      ez = 0;
    }
    return sf_pack(sign, ez, (sf_u64)diff << shift);
  }
  ma <<= 10;
  mb <<= 10;
  sf_i64 ez;
  sf_u64 z;
  if (d < 0) {
    sign ^= 1;
    if (eb == 0x7ff) return mb ? SF_NAN64 : sf_pack(sign, 0x7ff, 0);
    ma += ea ? (SF_ONE << 62) : ma;
    ma = sf_jam(ma, -d);
    mb |= SF_ONE << 62;
    ez = eb;
    z = mb - ma;
  } else {
    if (ea == 0x7ff) return ma ? SF_NAN64 : a;
    mb += eb ? (SF_ONE << 62) : mb;
    mb = sf_jam(mb, d);
    ma |= SF_ONE << 62;
    ez = ea;
    z = ma - mb;
  }
  return sf_norm_round_pack(sign, ez - 1, z);
}

inline sf_u64 sf_add(sf_u64 a, sf_u64 b) {
  sf_u64 sa = a >> 63;
  return sa == (b >> 63) ? sf_add_mags(a, b, sa) : sf_sub_mags(a, b, sa);
}

inline sf_u64 sf_sub(sf_u64 a, sf_u64 b) { return sf_add(a, b ^ (SF_ONE << 63)); }

// (a * b) >> 64, with the low half ORed into bit 0.
inline sf_u64 sf_mul_jam(sf_u64 a, sf_u64 b) {
  sf_u64 a0 = a & 0xffffffff, a1 = a >> 32, b0 = b & 0xffffffff, b1 = b >> 32;
  sf_u64 p00 = a0 * b0, p01 = a0 * b1, p10 = a1 * b0, p11 = a1 * b1;
  sf_u64 mid = (p00 >> 32) + (p01 & 0xffffffff) + (p10 & 0xffffffff);
  sf_u64 lo = (mid << 32) | (p00 & 0xffffffff);
  sf_u64 hi = p11 + (p01 >> 32) + (p10 >> 32) + (mid >> 32);
  return hi | (sf_u64)(lo != 0);
}

inline sf_u64 sf_mul(sf_u64 a, sf_u64 b) {
  sf_u64 sign = (a ^ b) >> 63;
  sf_i64 ea = sf_exp(a), eb = sf_exp(b);
  sf_u64 ma = sf_sig(a), mb = sf_sig(b);
  if (ea == 0x7ff || eb == 0x7ff) {
    if ((ea == 0x7ff && ma) || (eb == 0x7ff && mb)) return SF_NAN64;
    // infinity times the other operand: NaN when that is zero
    sf_u64 other = ea == 0x7ff ? b << 1 : a << 1;
    return other ? sf_pack(sign, 0x7ff, 0) : SF_NAN64;
  }
  if (ea == 0) {
    if (ma == 0) return sign << 63;
    ea = sf_normalize(&ma);
  }
  if (eb == 0) {
    if (mb == 0) return sign << 63;
    eb = sf_normalize(&mb);
  }
  sf_i64 ez = ea + eb - 0x3ff;
  ma = (ma | (SF_ONE << 52)) << 10;
  mb = (mb | (SF_ONE << 52)) << 11;
  sf_u64 z = sf_mul_jam(ma, mb);
  if (z < (SF_ONE << 62)) {
    ez--;
    z <<= 1;
  }
  return sf_round_pack(sign, ez, z);
}

inline sf_u64 sf_div(sf_u64 a, sf_u64 b) {
  sf_u64 sign = (a ^ b) >> 63;
  sf_i64 ea = sf_exp(a), eb = sf_exp(b);
  sf_u64 ma = sf_sig(a), mb = sf_sig(b);
  if (ea == 0x7ff) {
    if (ma || eb == 0x7ff) return SF_NAN64;  // NaN, or inf / inf
    return sf_pack(sign, 0x7ff, 0);
  }
  if (eb == 0x7ff) return mb ? SF_NAN64 : sign << 63;
  if (eb == 0 && mb == 0) return (ea == 0 && ma == 0) ? SF_NAN64 : sf_pack(sign, 0x7ff, 0);
  if (ea == 0) {
    if (ma == 0) return sign << 63;
    ea = sf_normalize(&ma);
  }
  if (eb == 0) eb = sf_normalize(&mb);
  sf_i64 ez = ea - eb + 0x3fe;
  ma |= SF_ONE << 52;
  mb |= SF_ONE << 52;
  if (ma < mb) {
    ez--;
    ma <<= 1;
  }
  // 63 quotient bits, the first one set; the remainder is the sticky bit
  sf_u64 q = 0;
  for (int i = 0; i < 63; i++) {
    q <<= 1;
    if (ma >= mb) {
      ma -= mb;
      q |= 1;
    }
    ma <<= 1;
  }
  return sf_round_pack(sign, ez, q | (sf_u64)(ma != 0));
}

inline sf_u64 sf_sqrt(sf_u64 a) {
  sf_i64 ea = sf_exp(a);
  sf_u64 ma = sf_sig(a);
  if (ea == 0x7ff) return (ma || (a >> 63)) ? SF_NAN64 : a;
  if (a >> 63) return (ea == 0 && ma == 0) ? a : SF_NAN64;  // sqrt(-0) is -0
  if (ea == 0) {
    if (ma == 0) return a;
    ea = sf_normalize(&ma);
  }
  sf_i64 e = ea - 0x3ff;
  sf_u64 m = ma | (SF_ONE << 52);
  if (e & 1) {
    m <<= 1;
    e--;
  }
  // 55 bits of floor(sqrt(m * 2^56)), two radicand bits per step; the
  // remainder is the sticky bit
  sf_u64 root = 0, rem = 0;
  for (int i = 54; i >= 0; i--) {
    int p = 2 * i;
    sf_u64 two = p >= 56 ? (m >> (p - 56)) & 3 : 0;
    rem = (rem << 2) | two;
    sf_u64 trial = (root << 2) | 1;
    root <<= 1;
    if (rem >= trial) {
      rem -= trial;
      root |= 1;
    }
  }
  return sf_round_pack(0, e / 2 + 0x3fe, (root << 8) | (sf_u64)(rem != 0));
}

inline bool sf_is_nan(sf_u64 a) { return (a & ~(SF_ONE << 63)) > ((sf_u64)0x7ff << 52); }

inline bool sf_eq(sf_u64 a, sf_u64 b) {
  if (sf_is_nan(a) || sf_is_nan(b)) return false;
  return a == b || ((a | b) << 1) == 0;
}

inline bool sf_lt(sf_u64 a, sf_u64 b) {
  if (sf_is_nan(a) || sf_is_nan(b)) return false;
  bool sa = a >> 63, sb = b >> 63;
  if (sa != sb) return sa && ((a | b) << 1) != 0;
  return a != b && (sa != (a < b));
}

inline bool sf_le(sf_u64 a, sf_u64 b) {
  if (sf_is_nan(a) || sf_is_nan(b)) return false;
  bool sa = a >> 63, sb = b >> 63;
  if (sa != sb) return sa || ((a | b) << 1) == 0;
  return a == b || (sa != (a < b));
}

// A binary f64 operation by opcode, as mithril_core::float::f64_op.
inline sf_u64 sf_f64_op(int op, sf_u64 a, sf_u64 b) {
  switch (op) {
  case 0: return sf_add(a, b);
  case 1: return sf_sub(a, b);
  case 2: return sf_mul(a, b);
  default: return sf_div(a, b);
  }
}

// ---- binary32 through binary64 ----
//
// binary64 carries more than twice binary32's precision plus two bits, so
// an f32 add, sub, mul, div or sqrt computed in f64 and rounded once to f32
// is the correctly rounded f32 result, subnormals included.

// The f64 holding f32 `x` exactly.
inline sf_u64 sf_f32_to_f64(sf_u32 x) {
  sf_u64 sign = (sf_u64)(x >> 31) << 63;
  sf_i64 e = (x >> 23) & 0xff;
  sf_u64 m = x & 0x7fffff;
  if (e == 0xff) return m ? SF_NAN64 : sign | ((sf_u64)0x7ff << 52);
  if (e == 0) {
    if (m == 0) return sign;
    int s = sf_clz64(m) - 40;  // the leading bit to bit 23
    m = (m << s) & 0x7fffff;
    e = 1 - s;
  }
  return sign | ((sf_u64)(e + 896) << 52) | (m << 29);
}

// The f32 nearest f64 `a` (ties to even); a NaN gives the canonical NaN.
inline sf_u32 sf_f64_to_f32(sf_u64 a) {
  sf_u32 sign = (sf_u32)(a >> 63) << 31;
  sf_i64 e = sf_exp(a);
  sf_u64 m = sf_sig(a);
  if (e == 0x7ff) return m ? SF_NAN32 : sign | 0x7f800000;
  if (e == 0) return sign;  // an f64 subnormal is far below half the least f32
  sf_i64 e32 = e - 896;
  if (e32 >= 0xff) return sign | 0x7f800000;
  sf_u64 sig = m | (SF_ONE << 52);
  sf_i64 shift = 29;
  if (e32 <= 0) {
    shift += 1 - e32;
    e32 = 0;
  }
  if (shift > 54) return sign;  // below half the least subnormal
  sf_u64 q = sig >> shift, rem = sig & ((SF_ONE << shift) - 1), mid = SF_ONE << (shift - 1);
  if (rem > mid || (rem == mid && (q & 1))) q++;
  // a carry out of the significand raises the exponent (packing by addition)
  sf_u64 bits = e32 == 0 ? q : ((sf_u64)(e32 - 1) << 23) + q;
  if (bits >= 0x7f800000) return sign | 0x7f800000;
  return sign | (sf_u32)bits;
}

// An f32 operation by opcode (0 add, 1 sub, 2 mul, 3 div, 4 sqrt), exact
// on subnormal operands and results.
inline sf_u32 sf_f32_op(int op, sf_u32 x, sf_u32 y) {
  sf_u64 a = sf_f32_to_f64(x), b = sf_f32_to_f64(y);
  return sf_f64_to_f32(op == 4 ? sf_sqrt(a) : sf_f64_op(op, a, b));
}

#endif
