// Scalar operations of the Metal backend, bit-equal to the CPU's
// (mithril_core::float, mithril_front::core::int_op). Values travel as i64:
// an f32 or f16 as its bit pattern, an f64 as its binary64 bits.
// Follows soft64.h (sf_*), which the backend puts in front of this file.

#pragma METAL fp contract(off)

typedef long i64;
typedef ulong u64;
typedef uint u32;

// ---- ints: 64-bit, wrapping (signed overflow is undefined in MSL, so the
// arithmetic runs unsigned) ----

inline i64 i_add(i64 a, i64 b) { return (i64)((u64)a + (u64)b); }
inline i64 i_sub(i64 a, i64 b) { return (i64)((u64)a - (u64)b); }
inline i64 i_mul(i64 a, i64 b) { return (i64)((u64)a * (u64)b); }
// shift counts wrap mod 64, as the CPU's wrapping shifts
inline i64 i_shl(i64 a, i64 b) { return (i64)((u64)a << ((u32)b & 63)); }
inline i64 i_shr(i64 a, i64 b) { return a >> ((u32)b & 63); }

// Division truncates; MIN / -1 wraps to MIN. A zero divisor gives 0, as
// on the CUDA device (the CPU reports an error instead).
inline i64 idiv(i64 a, i64 b) {
  if (b == 0) return 0;
  if (b == -1) return i_sub(0, a);
  return a / b;
}
inline i64 irem(i64 a, i64 b) {
  if (b == 0 || b == -1) return 0;
  return a % b;
}
inline i64 floor_div(i64 a, i64 b) {
  i64 q = idiv(a, b), r = irem(a, b);
  return (r != 0 && ((r < 0) != (b < 0))) ? q - 1 : q;
}
inline i64 py_mod(i64 a, i64 b) {
  i64 r = irem(a, b);
  return (r != 0 && ((r < 0) != (b < 0))) ? r + b : r;
}

// ---- binary32: hardware on normal values; the GPU flushes subnormal
// operands and results to zero, so an operation with a subnormal operand,
// or whose result has a zero exponent field (zero or a flushed subnormal),
// is recomputed exactly through software binary64 ----

inline float f32b(i64 x) { return as_type<float>((u32)x); }
inline i64 f32i(float x) { return (i64)as_type<u32>(x); }
inline bool f32_tiny(u32 x) { return (x & 0x7f800000u) == 0 && (x & 0x7fffffu) != 0; }

inline i64 f32_checked(int op, i64 a, i64 b, float r) {
  u32 x = (u32)a, y = (u32)b, z = as_type<u32>(r);
  if (f32_tiny(x) || f32_tiny(y) || (z & 0x7f800000u) == 0) return (i64)sf_f32_op(op, x, y);
  return (i64)z;
}

inline i64 f32_add(i64 a, i64 b) { return f32_checked(0, a, b, f32b(a) + f32b(b)); }
inline i64 f32_sub(i64 a, i64 b) { return f32_checked(1, a, b, f32b(a) - f32b(b)); }
inline i64 f32_mul(i64 a, i64 b) { return f32_checked(2, a, b, f32b(a) * f32b(b)); }
inline i64 f32_div(i64 a, i64 b) { return f32_checked(3, a, b, metal::precise::divide(f32b(a), f32b(b))); }
// a square root is never subnormal: only the operand needs the check
inline i64 f32_sqrt(i64 a) {
  if (f32_tiny((u32)a)) return (i64)sf_f32_op(4, (u32)a, 0);
  return f32i(metal::precise::sqrt(f32b(a)));
}

// comparisons on the exact widened values (the hardware would compare
// flushed subnormals as zero)
inline i64 f32_lt(i64 a, i64 b) { return sf_lt(sf_f32_to_f64((u32)a), sf_f32_to_f64((u32)b)) ? 1 : 0; }
inline i64 f32_le(i64 a, i64 b) { return sf_le(sf_f32_to_f64((u32)a), sf_f32_to_f64((u32)b)) ? 1 : 0; }

inline i64 f32_from_u32(i64 a) { return f32i((float)(u32)a); }
// toward zero; NaN, negative or >= 2^32 give 0 (a subnormal truncates to 0)
inline i64 f32_to_u32(i64 a) {
  float x = f32b(a);
  if (x != x || x < 0.0f || x >= 4294967296.0f) return 0;
  return (i64)(u32)x;
}
inline i64 f32_canon(i64 a) { return ((u32)a & 0x7fffffffu) > 0x7f800000u ? (i64)SF_NAN32 : a; }

// ---- binary16 conversions, in integer arithmetic ----

inline i64 f16_to_f32(i64 x) {
  u32 h = (u32)x & 0xffff, sign = (h & 0x8000) << 16, exp = (h >> 10) & 0x1f, man = h & 0x3ff;
  if (exp == 0x1f) return (i64)(sign | 0x7f800000u | (man << 13));
  if (exp != 0) return (i64)(sign | ((exp + 112) << 23) | (man << 13));
  if (man == 0) return (i64)sign;
  u32 p = 31 - metal::clz(man);
  return (i64)(sign | ((p + 103) << 23) | ((man << (23 - p)) & 0x7fffffu));
}

inline i64 f32_to_f16(i64 x) {
  u32 f = (u32)x, sign = (f >> 16) & 0x8000, a = f & 0x7fffffffu;
  if (a > 0x7f800000u) return 0x7e00;
  if (a >= 0x477ff000u) return sign | 0x7c00;
  if (a >= 0x38800000u) return sign | ((a + 0xfff + ((a >> 13) & 1) - 0x38000000u) >> 13);
  if (a <= 0x33000000u) return sign;
  u32 m = (a & 0x7fffffu) | 0x800000u, shift = 126 - (a >> 23);
  u32 q = m >> shift, rem = m & ((1u << shift) - 1), mid = 1u << (shift - 1);
  return sign | (q + ((rem > mid || (rem == mid && (q & 1))) ? 1 : 0));
}

// ---- binary64: software (the GPU has no double) ----

inline i64 f64_op(int op, i64 a, i64 b) { return (i64)sf_f64_op(op, (u64)a, (u64)b); }
inline bool f64_lt(i64 a, i64 b) { return sf_lt((u64)a, (u64)b); }
inline bool f64_le(i64 a, i64 b) { return sf_le((u64)a, (u64)b); }
inline bool f64_eq(i64 a, i64 b) { return sf_eq((u64)a, (u64)b); }
