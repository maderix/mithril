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
#define ARR_FLAGS ((u64)3 << 62)
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
  arr_block(a)[2 + i] = v;
  return a;
}
inline u64 arr_get_r(u64 a, i64 i) { return arr_get_n(a, arr_len_of(a), i); }
inline u64 arr_set_u(u64 a, i64 i, u64 v) { return arr_set_n(a, arr_len_of(a), i, v); }

// ---- work and depth: fuel[0] counts work (a leaf never suspends: it is
// not observed), fuel[1] the guarded calls open, fuel[2] the most the
// pipeline's stack holds; past it a leaf faults (DEEP_FAULT stays set) and
// the host retries deeper or runs the request on the CPU ----

#define DEEP_FAULT ((i64)1 << 40)

inline void work_fuel(thread i64 *fuel, i64 n) { fuel[0] -= n; }
inline bool deep_faulted(thread i64 *fuel) { return (fuel[1] & DEEP_FAULT) != 0; }
// after a fault every guarded call returns at once, and every loop stops
// (the printer checks deep_faulted at each iteration): the leaf ends
inline bool deep_enter(thread i64 *fuel) {
  if (deep_faulted(fuel) || (fuel[1] & 0xffff) >= fuel[2]) {
    fuel[1] |= DEEP_FAULT;
    return true;
  }
  fuel[1] += 1;
  return false;
}
inline void deep_leave(thread i64 *fuel) { fuel[1] -= 1; }
inline void mith_unreachable() {}
