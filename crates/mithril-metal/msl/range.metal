// Range launches on the GPU: the helpers the range leaves call (arrays,
// work, the depth guard) and the kernel that runs one index of a proven
// fold per thread. Follows ops.metal; the program's leaves follow this file
// (mithril_codegen::cprint::msl_leaves), the kernel after them.

typedef ulong usize;
typedef ushort u16;
typedef uchar u8;

// ---- arrays: the CPU's block layout ([refcount, length | flags,
// elements...]) at a device address; a handle is (T_ARR << 56) | address ----

#define M56 ((u64)0xffffffffffffff)
// a fill's staged copy has a write map after its elements: one byte per
// element, set where the GPU writes (the host copies those back)
#define ARR_MAP ((u64)1 << 61)
#define ARR_FLAGS ((u64)7 << 61)
// written over a block's refcount word when a leaf indexes out of bounds:
// the host then runs the request on the CPU, which reports the fault
#define ARR_FAULT ((u64)1)

inline device u64 *arr_block(u64 p) { return reinterpret_cast<device u64 *>(p & M56); }
inline usize arr_len_of(u64 p) { return arr_block(p)[1] & ~ARR_FLAGS; }
inline u64 arr_fault(u64 p) {
  arr_block(p)[0] = ARR_FAULT;
  return 0;
}
inline u64 arr_get_n(u64 a, usize n, i64 i) {
  if ((u64)i >= n) return arr_fault(a);
  return arr_block(a)[2 + i];
}
inline u64 arr_set_n(u64 a, usize n, i64 i, u64 v) {
  if ((u64)i >= n) {
    arr_fault(a);
    return a;
  }
  device u64 *b = arr_block(a);
  b[2 + i] = v;
  if (b[1] & ARR_MAP) reinterpret_cast<device uchar *>(b + 2 + n)[i] = 1;
  return a;
}
inline u64 arr_get_r(u64 a, i64 i) { return arr_get_n(a, arr_len_of(a), i); }
inline u64 arr_set_u(u64 a, i64 i, u64 v) { return arr_set_n(a, arr_len_of(a), i, v); }

// ---- work and depth: fuel[0] counts work (a leaf never suspends: it is
// not observed); fuel[1] holds DEEP_FAULT once a guarded call reaches
// DEEP_LIMIT nested levels (the leaves' level templates), and the host
// retries deeper or runs the request on the CPU ----

#define DEEP_FAULT ((i64)1 << 40)

inline void work_fuel(thread i64 *fuel, i64 n) { fuel[0] -= n; }
// after a fault every loop stops (the printer checks deep_faulted at each
// iteration) and nothing nests deeper: the leaf ends
inline bool deep_faulted(thread i64 *fuel) { return (fuel[1] & DEEP_FAULT) != 0; }
inline void mith_unreachable() {}

// ---- binary32 in a leaf: the fast pass runs it in hardware and marks the
// index wherever the GPU's flushing may have changed a result: a subnormal
// operand, or a zero result the exact operation need not give (operands of
// different magnitudes that sum to zero, nonzero factors, a nonzero
// numerator over a finite divisor). The host runs the marked indices again
// in the exact pass (F32_EXACT), with ops.metal's exact operations ----

#define F32_SUSPECT ((i64)1 << 41)

#ifdef F32_EXACT
inline i64 m_f32_add(thread i64 *fuel, i64 a, i64 b) { return f32_add(a, b); }
inline i64 m_f32_sub(thread i64 *fuel, i64 a, i64 b) { return f32_sub(a, b); }
inline i64 m_f32_mul(thread i64 *fuel, i64 a, i64 b) { return f32_mul(a, b); }
inline i64 m_f32_div(thread i64 *fuel, i64 a, i64 b) { return f32_div(a, b); }
inline i64 m_f32_sqrt(thread i64 *fuel, i64 a) { return f32_sqrt(a); }
inline i64 m_f32_lt(thread i64 *fuel, i64 a, i64 b) { return f32_lt(a, b); }
inline i64 m_f32_le(thread i64 *fuel, i64 a, i64 b) { return f32_le(a, b); }
#else
inline u32 f32_mag(u32 x) { return x & 0x7fffffffu; }
inline bool f32_zero_exp(u32 z) { return (z & 0x7f800000u) == 0; }
inline void f32_mark(thread i64 *fuel, bool suspect) { fuel[1] |= (i64)suspect << 41; }
inline i64 f32_sum(thread i64 *fuel, u32 x, u32 y, float r) {
  u32 z = as_type<u32>(r);
  f32_mark(fuel, f32_tiny(x) | f32_tiny(y) | (f32_zero_exp(z) & (f32_mag(x) != f32_mag(y))));
  return (i64)z;
}
inline i64 m_f32_add(thread i64 *fuel, i64 a, i64 b) { return f32_sum(fuel, (u32)a, (u32)b, f32b(a) + f32b(b)); }
inline i64 m_f32_sub(thread i64 *fuel, i64 a, i64 b) { return f32_sum(fuel, (u32)a, (u32)b, f32b(a) - f32b(b)); }
inline i64 m_f32_mul(thread i64 *fuel, i64 a, i64 b) {
  u32 x = (u32)a, y = (u32)b, z = as_type<u32>(f32b(a) * f32b(b));
  f32_mark(fuel, f32_tiny(x) | f32_tiny(y) | (f32_zero_exp(z) & (f32_mag(x) != 0) & (f32_mag(y) != 0)));
  return (i64)z;
}
inline i64 m_f32_div(thread i64 *fuel, i64 a, i64 b) {
  u32 x = (u32)a, y = (u32)b, z = as_type<u32>(metal::precise::divide(f32b(a), f32b(b)));
  f32_mark(fuel, f32_tiny(x) | f32_tiny(y) | (f32_zero_exp(z) & (f32_mag(x) != 0) & (f32_mag(y) < 0x7f800000u)));
  return (i64)z;
}
inline i64 m_f32_sqrt(thread i64 *fuel, i64 a) {
  f32_mark(fuel, f32_tiny((u32)a));
  return f32i(metal::precise::sqrt(f32b(a)));
}
inline i64 m_f32_lt(thread i64 *fuel, i64 a, i64 b) {
  f32_mark(fuel, f32_tiny((u32)a) | f32_tiny((u32)b));
  return f32b(a) < f32b(b) ? 1 : 0;
}
inline i64 m_f32_le(thread i64 *fuel, i64 a, i64 b) {
  f32_mark(fuel, f32_tiny((u32)a) | f32_tiny((u32)b));
  return f32b(a) <= f32b(b) ? 1 : 0;
}
#endif
