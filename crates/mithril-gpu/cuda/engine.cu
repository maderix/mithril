// mithril-gpu: the device runtime (program-independent).
//
// A generated program.cu does:
//     #define PROG_NRULES <n>
//     #include "engine.cu"
//     ... the program's tables (LIN, LIN_TUP, UNBOX_CID), its functions,
//     __device__ R prog_dive(...) and __device__ void prog_fire(...)
// This file supplies the same helper vocabulary the lowered IR (lir) names
// on the CPU (`mithril_rt::prelude`): cells, refcounts, records, delivery,
// dives, constructors, sharing, arrays, arithmetic; plus the kernels the
// host wave loop launches. Protocol mirrors the CPU engine (mithril-rt):
// the boot redex fires as rule 0, record 0 is the reserved ROOT result
// sink (parent address 0), a record whose pend counter reaches zero is
// queued (never fired inline) into its rule's bucket with the record index
// in the entry's third word.
//
// Memory safety after an abort: alloc returns cell 0 / the last record, and
// every cell/record read is clamped into the arena, so a poisoned run can
// produce garbage values but never an illegal access; the host reads the
// abort flag each wave and turns it into a clean error.

typedef unsigned long long u64;
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;
typedef long long i64;
typedef unsigned long long usize;

#define MAXLANES (1 << 16)
#define FREECAP 64

// abort codes (host maps each to a message); arena outranks the rest via
// atomicMax so the root cause wins.
#define AB_UNREACHABLE 1u
#define AB_ARENA 2u
#define AB_OOB 3u
#define AB_UNSUPPORTED 4u

struct Rec {
  int pend;
  unsigned short rule;
  unsigned short s;
  u32 d;
  u32 _pad;
  u64 parent; // rec_idx << 3 | slot
  u64 args[2];
};

struct Dev {
  u64 *nodes;  // ncap cell pairs
  u32 *rc;     // ncap refcounts (only read for non-linear constructors)
  Rec *recs;   // rcap records (0 = ROOT sink)
  u32 *nbump;  // cell bump (starts at 1: cell 0 reserved)
  u32 *rbump;  // record bump (starts at 1: rec 0 = ROOT)
  u32 *nfree;  // MAXLANES * FREECAP per-lane free lists
  u32 *nfreen; // MAXLANES free-list lengths
  u32 *nchunk; // MAXLANES * 2 (cur, end) bump chunks
  u32 *ovf;    // global overflow ring of freed cells (idx+1; 0 = empty)
  int *ovftop;
  u64 *ebuf;   // PROG_NRULES buckets of bcap 3-word entries
  u32 *blen;   // PROG_NRULES append counters (host drains by prefix)
  u32 *bdone;  // PROG_NRULES drained prefixes (shared with the host; a
               // fully drained bucket is recycled to 0 between waves)
  u64 *result; // [0] = delivered flag, [1] = ROOT value
  u32 *abortf;
  u64 *heap;   // array blocks: [rc, len|flags, elems..] (bump, never freed)
  u64 *hbump;
  u64 hcap;
  u32 ncap, rcap, bcap, ovfcap, chunksz, nrules;
  int fuel;    // per-dive budget
};

extern "C" {
__device__ Dev G;
__device__ u32 g_nrules = PROG_NRULES;
}

struct R {
  u64 v; // the value, or the suspension record when !ok
  bool ok;
};
template <int K> struct A { u64 a[K > 0 ? K : 1]; };
template <int K> struct RA { A<K> a; u64 rec; bool ok; };
struct P2 { u64 f0, f1; };
struct P2R { u64 f0, f1; u32 f2; };
// T<k> (native int tuples) are declared by the program for the widths it uses

__device__ void prog_fire(u32 rule, u64 e0, u64 e1, u64 e2);
__device__ R prog_dive(u32 f, const u64 *args, i64 *fuel);
__device__ bool lin(u16 k);
__device__ u32 unbox_cid(u64 slot);

__device__ inline void g_abort(u32 code) { atomicMax(G.abortf, code); }

__device__ inline u32 lane() {
  return (blockIdx.x * blockDim.x + threadIdx.x) & (MAXLANES - 1);
}

// ---- ports: tag:8 | payload:56; CON payload: addr:40 | ctor:12 | arity:4 ----

#define T_NUM 2ull
#define T_FLO 3ull
#define T_CON 4ull
#define T_LAM 6ull
#define T_ARR 14ull
#define TU 16ull
#define M56 0x00ffffffffffffffull
#define NONE 0xffffffffffffffffull
#define NOHOLE 0xffffffffu
#define NOTOK 0xffffffffu
#define ROOT 0ull

__device__ inline u64 tag(u64 p) { return p >> 56; }
__device__ inline u64 num(i64 v) { return (T_NUM << 56) | ((u64)v & M56); }
__device__ inline i64 as_i(u64 p) { return ((i64)(p << 8)) >> 8; }
__device__ inline i64 wrap56(i64 v) { return ((i64)((u64)v << 8)) >> 8; }
__device__ inline u64 ic(u64 slot, i64 v) { return ((TU + slot) << 56) | ((u64)v & M56); }
__device__ inline u64 con(u32 addr, u16 k, u8 ar) {
  return (T_CON << 56) | ((u64)addr << 16) | ((u64)(k & 0xfffu) << 4) | (u64)ar;
}
__device__ inline u32 con_addr(u64 p) { return (u32)((p >> 16) & ((1ull << 40) - 1)); }
__device__ inline u16 con_tag(u64 p) { return (u16)((p >> 4) & 0xfffu); }
__device__ inline u8 con_ar(u64 p) { return (u8)(p & 0xf); }
__device__ inline i64 sh(u64 p) { return (i64)(p << 8); }
__device__ inline u64 retag(i64 x) { return ((u64)x >> 8) | (T_NUM << 56); }
__device__ inline u64 rec_addr(u32 rec) { return (u64)rec << 3; }
__device__ inline i64 imax(i64 a, i64 b) { return a > b ? a : b; }
__device__ inline i64 sat_mul(i64 a, i64 b) {
  i64 hi = __mul64hi(a, b), lo = a * b;
  if ((hi == 0 && lo >= 0) || (hi == -1 && lo < 0)) return lo;
  return ((a < 0) != (b < 0)) ? (i64)0x8000000000000000ull : (i64)0x7fffffffffffffffull;
}
__device__ inline bool is_err(const R *r) { return !r->ok; }
__device__ inline u64 mith_unreachable() { g_abort(AB_UNREACHABLE); return 0; }

__device__ inline i64 floor_div(i64 a, i64 b) {
  if (b == 0) return 0;
  i64 q = a / b, r = a % b;
  return (r != 0 && ((r < 0) != (b < 0))) ? q - 1 : q;
}
__device__ inline i64 py_mod(i64 a, i64 b) {
  if (b == 0) return 0;
  i64 r = a % b;
  return (r != 0 && ((r < 0) != (b < 0))) ? r + b : r;
}
__device__ inline i64 idiv(i64 a, i64 b) { return b == 0 ? 0 : a / b; }

// binary32 on bit patterns (mithril_front::core::f32_prim)
__device__ inline float f32b(i64 x) { return __uint_as_float((u32)x); }
__device__ inline i64 f32i(float x) { return (i64)__float_as_uint(x); }
// _rn intrinsics: never contracted into fma, so device and CPU round alike
__device__ inline i64 f32_add(i64 a, i64 b) { return f32i(__fadd_rn(f32b(a), f32b(b))); }
__device__ inline i64 f32_sub(i64 a, i64 b) { return f32i(__fsub_rn(f32b(a), f32b(b))); }
__device__ inline i64 f32_mul(i64 a, i64 b) { return f32i(__fmul_rn(f32b(a), f32b(b))); }
__device__ inline i64 f32_div(i64 a, i64 b) { return f32i(__fdiv_rn(f32b(a), f32b(b))); }
__device__ inline i64 f32_sqrt(i64 a) { return f32i(__fsqrt_rn(f32b(a))); }
__device__ inline i64 f32_lt(i64 a, i64 b) { return f32b(a) < f32b(b) ? 1 : 0; }
__device__ inline i64 f32_from_u32(i64 a) { return f32i(__uint2float_rn((u32)a)); }
__device__ inline i64 f32_to_u32(i64 a) {
  float x = f32b(a);
  if (x != x || x < 0.0f || x >= 4294967296.0f) return 0;
  return (i64)(u32)x;
}

// ---- clamped cell access (safe even on garbage after an abort) ----

__device__ inline u32 nclamp(u32 i) { return i < G.ncap ? i : G.ncap - 1; }
__device__ inline u64 cell0(u32 i) { return G.nodes[2 * (u64)nclamp(i)]; }
__device__ inline u64 cell1(u32 i) { return G.nodes[2 * (u64)nclamp(i) + 1]; }
__device__ inline void setcell(u32 i, u64 a, u64 b) {
  u32 j = nclamp(i);
  G.nodes[2 * (u64)j] = a;
  G.nodes[2 * (u64)j + 1] = b;
}
__device__ inline void cell_set(u32 i, usize slot, u64 v) { G.nodes[2 * (u64)nclamp(i) + slot] = v; }

// ---- node arena: lane free list -> global overflow ring -> checked bump ----

__device__ u32 alloc_node(u64 a, u64 b) {
  u32 L = lane();
  if (G.nfreen[L]) {
    u32 i = G.nfree[L * FREECAP + --G.nfreen[L]];
    setcell(i, a, b);
    return i;
  }
  // Try the global overflow ring. Slot values are exchanged atomically, so a
  // node is never handed out twice; a lost race falls through to the bump
  // (worst case a node stays stranded in its slot until a later pop).
  int t = atomicSub(G.ovftop, 1) - 1;
  if (t >= 0 && (u32)t < G.ovfcap) {
    u32 v = atomicExch(&G.ovf[t], 0u);
    if (v) {
      setcell(v - 1, a, b);
      return v - 1;
    }
  }
  atomicAdd(G.ovftop, 1); // took no value: restore the counter
  u32 *ck = &G.nchunk[2 * L];
  if (ck[0] >= ck[1]) {
    u32 base = atomicAdd(G.nbump, G.chunksz);
    if (base >= G.ncap) {
      g_abort(AB_ARENA);
      return 0;
    }
    ck[0] = base;
    u64 end = (u64)base + G.chunksz;
    ck[1] = end > (u64)G.ncap ? G.ncap : (u32)end;
  }
  u32 i = ck[0]++;
  setcell(i, a, b);
  return i;
}

__device__ void free_node(u32 i) {
  if (i == 0 || i >= G.ncap)
    return;
  u32 L = lane();
  if (G.nfreen[L] < FREECAP) {
    G.nfree[L * FREECAP + G.nfreen[L]++] = i;
    return;
  }
  // lane list full: spill to the global overflow ring (fixes the spike leak)
  int t = atomicAdd(G.ovftop, 1);
  if (t >= 0 && (u32)t < G.ovfcap)
    atomicExch(&G.ovf[t], i + 1);
  else
    atomicSub(G.ovftop, 1); // ring full: drop (bounded leak, never unsafe)
}

__device__ inline u32 alloc2(u64 a, u64 b) { u32 i = alloc_node(a, b); G.rc[nclamp(i)] = 1; return i; }
__device__ inline void cell_free(u32 i) { free_node(i); }
__device__ inline void tok_free(u32 tok) { if (tok != NOTOK) free_node(tok); }
__device__ inline void rc_inc(u32 i) { atomicAdd(&G.rc[nclamp(i)], 1u); }
__device__ inline bool rc_dec(u32 i) { return atomicSub(&G.rc[nclamp(i)], 1u) == 1u; }
__device__ inline void rc_set1(u32 i) { G.rc[nclamp(i)] = 1; }
__device__ inline bool rc_unique(u32 i) { return G.rc[nclamp(i)] == 1; }

// ---- records / buckets / delivery ----

__device__ u32 alloc_rec(u16 rule, u32 pend, u32 d, u32 s, u64 parent) {
  u32 i = atomicAdd(G.rbump, 1);
  if (i >= G.rcap) {
    g_abort(AB_ARENA);
    return G.rcap - 1;
  }
  Rec &r = G.recs[i];
  r.pend = (int)pend;
  r.rule = (unsigned short)rule;
  r.s = (unsigned short)s;
  r.d = d;
  r.parent = parent;
  return i;
}

__device__ inline u32 rclamp(u32 i) { return i < G.rcap ? i : G.rcap - 1; }
__device__ inline u64 rec_parent(u32 rec) { return G.recs[rclamp(rec)].parent; }
__device__ inline u32 rec_d(u32 rec) { return G.recs[rclamp(rec)].d; }
__device__ inline u32 rec_s(u32 rec) { return G.recs[rclamp(rec)].s; }
__device__ inline void set_parent(u32 rec, u64 parent) { G.recs[rclamp(rec)].parent = parent; }
__device__ inline i64 fuel_of() { return (i64)G.fuel; }

__device__ void spawn3(u32 rule, u64 a, u64 b, u64 c) {
  u32 i = atomicAdd(&G.blen[rule], 1);
  if (i >= G.bcap) {
    g_abort(AB_ARENA);
    return;
  }
  u64 *e = &G.ebuf[((u64)rule * G.bcap + i) * 3];
  e[0] = a;
  e[1] = b;
  e[2] = c;
}

__device__ inline void ready_rec(u32 rec) {
  Rec &r = G.recs[rclamp(rec)];
  spawn3(r.rule, r.args[0], r.args[1], (u64)rec);
}

__device__ void deliver(u64 parent, u64 val) {
  u32 ri = (u32)(parent >> 3);
  u32 slot = (u32)parent & 1u;
  if (ri == 0) { // ROOT sink
    G.result[1] = val;
    __threadfence();
    G.result[0] = 1;
    return;
  }
  if (ri >= G.rcap)
    ri = G.rcap - 1;
  Rec &r = G.recs[ri];
  r.args[slot] = val;
  __threadfence();
  if (atomicSub(&r.pend, 1) == 1)
    spawn3(r.rule, r.args[0], r.args[1], (u64)ri);
}

// Spawn a saturated call: arity <= 2 rides in (a, b); wider calls put
// arg0 in `a` and chain args[1..] through cells in `b` (addr+1, 0 = end).
__device__ void spawn_call(u16 rule, const u64 *args, int n, u64 parent) {
  u64 a = 0, b = 0;
  if (n == 1) {
    a = args[0];
  } else if (n == 2) {
    a = args[0];
    b = args[1];
  } else if (n > 2) {
    u64 ch = 0;
    for (int k = n - 1; k >= 1; k--)
      ch = (u64)alloc_node(args[k], ch) + 1;
    a = args[0];
    b = ch;
  }
  spawn3(rule, a, b, parent);
}

// Pop the head value of a `[value, next]` spill chain (`ch` = addr + 1;
// 0 = end), freeing its cell.
__device__ inline u64 pop_chain(u64 *ch) {
  u32 i = (u32)(*ch - 1);
  u64 v = cell0(i);
  *ch = cell1(i);
  free_node(i);
  return v;
}

// Dive `f` (args[0] = the destination) and deliver its result there; a
// suspended dive's residue root is attached to the destination.
__device__ void dive_to(u16 f, const u64 *args, int n) {
  i64 fuel = (i64)G.fuel;
  R r = prog_dive(f, args, &fuel);
  if (r.ok)
    deliver(args[0], r.v);
  else if (args[0] != NONE)
    set_parent((u32)r.v, args[0]);
}

// Dive `f` with no destination (args[0] = NONE): ok = the value, else the
// root record of its residue, whose parent the caller sets.
__device__ R dive_res(u16 f, const u64 *args, int n) {
  i64 fuel = (i64)G.fuel;
  return prog_dive(f, args, &fuel);
}

// ---- TRMC holes ----

__device__ inline void hole_link(u64 *head, u32 *hole, u64 p) {
  if (*hole == NOHOLE)
    *head = p;
  else
    cell_set(*hole, 1, p);
  *hole = con_addr(p);
}
__device__ inline u64 hole_fill(u64 head, u32 hole, u64 v) {
  if (hole == NOHOLE) return v;
  cell_set(hole, 1, v);
  return head;
}
__device__ u64 hole_wrap(u64 r, u64 head, u32 hole, u16 rule) {
  if (hole == NOHOLE) return r;
  u32 hr = alloc_rec(rule, 1, con_addr(head), hole, NONE);
  set_parent((u32)r, rec_addr(hr));
  return (u64)hr;
}

// ---- fold estimates (device globals declared by the program) ----

__device__ inline i64 atomic_load(i64 *a) { return *(volatile i64 *)a; }
__device__ inline void atomic_store(i64 *a, i64 v) { *(volatile i64 *)a = v; }
__device__ inline void atomic_max(i64 *a, i64 v) { atomicMax(a, v); }
__device__ inline bool flag_load(int *a) { return *(volatile int *)a != 0; }
__device__ inline void flag_store(int *a, bool v) { *(volatile int *)a = v ? 1 : 0; }

// ---- values: constructors, sharing, arrays, arithmetic ----
// (mirrors mithril_rt::prelude, generic over the program's `lin`)

__device__ u64 dup_val(u64 p);
__device__ void free_val(u64 p);
__device__ void free_val_slow(u64 p);

__device__ inline u32 calloc(u16 k, u64 a, u64 b) {
  u32 i = alloc_node(a, b);
  if (!lin(k)) G.rc[nclamp(i)] = 1;
  return i;
}
__device__ inline u64 mk_con1(u16 k, u64 f0) { u32 a = calloc(k, f0, 0); return con(a, k, 1); }
__device__ inline u64 mk_con2(u16 k, u64 f0, u64 f1) { u32 a = calloc(k, f0, f1); return con(a, k, 2); }
// arity <= 2 direct, wider ctors chain cells (slot 0 = field, slot 1 =
// continuation con); the arity nibble saturates at 15
__device__ u64 mk_con(u16 k, const u64 *fs, int n) {
  if (n == 0) return con(0, k, 0);
  if (n <= 2) {
    u32 a = calloc(k, fs[0], n == 2 ? fs[1] : 0);
    return con(a, k, (u8)n);
  }
  u32 a = calloc(k, fs[n - 2], fs[n - 1]);
  u64 chain = con(a, k, 2);
  int i = n - 2;
  while (i > 0) {
    i -= 1;
    u32 b = calloc(k, fs[i], chain);
    int ar = n - i;
    chain = con(b, k, (u8)(ar < 15 ? ar : 15));
  }
  return chain;
}
__device__ inline u64 mk_con2r(u32 tok, u16 k, u64 f0, u64 f1) {
  if (tok == NOTOK) return mk_con2(k, f0, f1);
  setcell(tok, f0, f1);
  if (!lin(k)) rc_set1(tok);
  return con(tok, k, 2);
}
// field i of a constructor value (walks the >2-arity chain)
__device__ u64 field(u64 p, usize i) {
  for (;;) {
    u8 ar = con_ar(p);
    u32 a = con_addr(p);
    if (ar > 2) {
      if (i == 0) return cell0(a);
      p = cell1(a);
      i -= 1;
    } else {
      return i == 0 ? cell0(a) : cell1(a);
    }
  }
}
__device__ inline P2 consume2k(u64 p, u16 k) {
  u32 a = con_addr(p);
  u64 c0 = cell0(a), c1 = cell1(a);
  if (lin(k) || rc_unique(a)) {
    free_node(a);
    return P2{c0, c1};
  }
  u64 f0 = dup_val(c0), f1 = dup_val(c1);
  rc_dec(a);
  return P2{f0, f1};
}
__device__ inline P2R consume2r(u64 p, u16 k) {
  u32 a = con_addr(p);
  u64 c0 = cell0(a), c1 = cell1(a);
  if (lin(k) || rc_unique(a)) return P2R{c0, c1, a};
  u64 f0 = dup_val(c0), f1 = dup_val(c1);
  rc_dec(a);
  return P2R{f0, f1, NOTOK};
}
template <int N> __device__ A<N> consume_chain(u64 p, u16 k) {
  A<N> out;
  u32 a = con_addr(p);
  if (N == 0) return out;
  if (lin(k) || rc_unique(a)) {
    u32 cur = a;
    for (int i = 0; i < N - 2; i++) {
      u64 c0 = cell0(cur), c1 = cell1(cur);
      free_node(cur);
      out.a[i] = c0;
      cur = con_addr(c1);
    }
    u64 c0 = cell0(cur), c1 = cell1(cur);
    free_node(cur);
    if (N >= 2) {
      out.a[N - 2] = c0;
      out.a[N - 1] = c1;
    } else {
      out.a[0] = c0;
    }
  } else {
    for (int i = 0; i < N; i++) out.a[i] = dup_val(field(p, i));
    free_val(p);
  }
  return out;
}
// last use of a boxed value that is only projected: move field i out
__device__ u64 take_field(u64 p, usize i) {
  u32 root = con_addr(p);
  if (!lin(con_tag(p)) && !rc_unique(root)) {
    u64 f = dup_val(field(p, i));
    free_val(p);
    return f;
  }
  u64 q = p, out = 0;
  usize idx = 0;
  for (;;) {
    u8 ar = con_ar(q);
    u32 ca = con_addr(q);
    u64 c0 = cell0(ca), c1 = cell1(ca);
    free_node(ca);
    if (ar > 2) {
      if (idx == i) out = c0; else free_val(c0);
      idx += 1;
      q = c1;
    } else {
      if (ar >= 1) { if (idx == i) out = c0; else free_val(c0); }
      if (ar >= 2) { if (idx + 1 == i) out = c1; else free_val(c1); }
      return out;
    }
  }
}
template <int K> __device__ A<K> untup(u64 p) {
  A<K> out;
  if (!lin(con_tag(p)) && !rc_unique(con_addr(p))) {
    for (int i = 0; i < K; i++) out.a[i] = dup_val(field(p, i));
    free_val(p);
    return out;
  }
  u64 q = p;
  int idx = 0;
  for (;;) {
    u8 ar = con_ar(q);
    u32 ca = con_addr(q);
    u64 c0 = cell0(ca), c1 = cell1(ca);
    free_node(ca);
    if (ar > 2) {
      out.a[idx++] = c0;
      q = c1;
    } else {
      if (ar >= 1) out.a[idx] = c0;
      if (ar >= 2) out.a[idx + 1] = c1;
      return out;
    }
  }
}

// ---- floats: boxed f64 bits in a cell ----

__device__ inline u64 flo(double x) {
  u32 a = alloc2(__double_as_longlong(x), 0);
  return (T_FLO << 56) | (u64)a;
}
__device__ inline double flo_val(u64 p) { return __longlong_as_double(cell0((u32)(p & M56))); }

// ---- arrays: [rc, len|flags, elems..] blocks on the device heap ----

#define ARR_BOXED (1ull << 63)
#define ARR_RAW (1ull << 62)
__device__ inline u64 *arr_block(u64 p) { return &G.heap[p & M56]; }
__device__ inline usize arr_len_of(u64 p) { return arr_block(p)[1] & ~(ARR_BOXED | ARR_RAW); }
__device__ inline u64 *arr_elems(u64 p) { return arr_block(p) + 2; }
__device__ inline bool arr_raw(u64 p) { return (arr_block(p)[1] & ARR_RAW) != 0; }
__device__ inline bool arr_boxed(u64 p) { return (arr_block(p)[1] & ARR_BOXED) != 0; }
__device__ inline bool is_heap(u64 v) { u64 t = tag(v); return t == T_CON || t == T_FLO || t == T_ARR || t == T_LAM; }
__device__ inline void arr_mark_boxed(u64 a, u64 v) { if (is_heap(v)) arr_block(a)[1] |= ARR_BOXED; }
__device__ inline u64 arr_elem(u64 p, usize k) { u64 e = arr_elems(p)[k]; return arr_raw(p) ? retag((i64)e) : e; }
__device__ u64 arr_alloc_fill(usize n, u64 fill) {
  u64 base = atomicAdd(G.hbump, n + 2);
  if (base + n + 2 > G.hcap) {
    g_abort(AB_ARENA);
    return (T_ARR << 56) | 0;
  }
  u64 *b = &G.heap[base];
  b[0] = 1;
  b[1] = n;
  for (usize k = 0; k < n; k++) b[2 + k] = fill;
  return (T_ARR << 56) | base;
}
__device__ inline u64 arr_alloc(usize n) { return arr_alloc_fill(n, 0); }
__device__ inline void arr_free_block(u64 p) { (void)p; } // bump heap: never freed
__device__ inline void arr_oob(i64 i, usize n) { (void)i; (void)n; g_abort(AB_OOB); }
__device__ inline u64 arr_new_raw(i64 n, i64 x) {
  if (n < 0) { g_abort(AB_OOB); n = 0; }
  u64 p = arr_alloc_fill((usize)n, (u64)x);
  arr_block(p)[1] |= ARR_RAW;
  return p;
}
__device__ void arr_unraw(u64 a) {
  usize n = arr_len_of(a);
  for (usize k = 0; k < n; k++) arr_elems(a)[k] = retag((i64)arr_elems(a)[k]);
  arr_block(a)[1] &= ~ARR_RAW;
}
__device__ inline u64 arr_get_r(u64 a, i64 i) {
  usize n = arr_len_of(a);
  if (i < 0 || (usize)i >= n) { arr_oob(i, n); return 0; }
  return arr_elems(a)[i];
}
__device__ inline u64 arr_get_i(u64 a, i64 i) { return retag((i64)arr_get_r(a, i)); }
__device__ inline u64 arr_set_u(u64 a, i64 i, u64 v) {
  usize n = arr_len_of(a);
  if (i < 0 || (usize)i >= n) { arr_oob(i, n); return a; }
  arr_elems(a)[i] = v;
  return a;
}
__device__ inline u64 arr_get_n(u64 a, usize n, i64 i) {
  if ((u64)i >= n) { arr_oob(i, n); return 0; }
  return arr_elems(a)[i];
}
__device__ inline u64 arr_set_n(u64 a, usize n, i64 i, u64 v) {
  if ((u64)i >= n) { arr_oob(i, n); return a; }
  arr_elems(a)[i] = v;
  return a;
}
__device__ inline u64 arr_new_i(i64 n, u64 v) { return arr_new_raw(n, sh(v)); }
__device__ inline u64 arr_rc_load(u64 p) { return *(volatile u64 *)arr_block(p); }
__device__ u64 arr_copy(u64 a) {
  usize n = arr_len_of(a);
  u64 b = arr_alloc(n);
  if (arr_boxed(a)) {
    for (usize k = 0; k < n; k++) arr_elems(b)[k] = dup_val(arr_elems(a)[k]);
    arr_block(b)[1] |= ARR_BOXED;
  } else {
    for (usize k = 0; k < n; k++) arr_elems(b)[k] = arr_elems(a)[k];
    if (arr_raw(a)) arr_block(b)[1] |= ARR_RAW;
  }
  free_val(a);
  return b;
}
__device__ inline u64 arr_own(u64 a) { return arr_rc_load(a) == 1 ? a : arr_copy(a); }
__device__ u64 arr_new(i64 n, u64 v) {
  if (n < 0) { g_abort(AB_OOB); n = 0; }
  if (tag(v) == T_NUM) return arr_new_raw(n, sh(v));
  usize un = (usize)n;
  if (!is_heap(v)) return arr_alloc_fill(un, v);
  u64 p = arr_alloc(un);
  for (usize k = 0; k < un; k++) arr_elems(p)[k] = (k + 1 == un) ? v : dup_val(v);
  if (un == 0) free_val(v); else arr_mark_boxed(p, v);
  return p;
}
__device__ inline u64 arr_get(u64 a, i64 i) {
  usize n = arr_len_of(a);
  if (i < 0 || (usize)i >= n) { arr_oob(i, n); return 0; }
  if (arr_raw(a)) return arr_elem(a, (usize)i);
  return dup_val(arr_elems(a)[i]);
}
__device__ u64 arr_set(u64 a, i64 i, u64 v) {
  usize n = arr_len_of(a);
  if (i < 0 || (usize)i >= n) { arr_oob(i, n); return a; }
  a = arr_rc_load(a) == 1 ? a : arr_copy(a);
  if (arr_raw(a)) {
    if (tag(v) == T_NUM) {
      arr_elems(a)[i] = (u64)sh(v);
      return a;
    }
    arr_unraw(a);
  }
  free_val(arr_elems(a)[i]);
  arr_elems(a)[i] = v;
  arr_mark_boxed(a, v);
  return a;
}
__device__ inline u64 arr_set_i(u64 a, i64 i, u64 v) {
  usize n = arr_len_of(a);
  if (i < 0 || (usize)i >= n) { arr_oob(i, n); return a; }
  a = arr_rc_load(a) == 1 ? a : arr_copy(a);
  arr_elems(a)[i] = (u64)sh(v);
  return a;
}
__device__ void arr_drop(u64 p) {
  if (atomicAdd((unsigned long long *)arr_block(p), 0xffffffffffffffffull) != 1ull) return;
  if (arr_boxed(p)) {
    usize n = arr_len_of(p);
    for (usize k = 0; k < n; k++) free_val(arr_elems(p)[k]);
  }
  arr_free_block(p);
}

// ---- closures: the net region runs on the device in stage 3 ----

__device__ inline u64 dup_closure(u64 p) { g_abort(AB_UNSUPPORTED); return p; }
__device__ inline void drop_closure(u64 p) { (void)p; g_abort(AB_UNSUPPORTED); }
__device__ inline R apply(u64 f, u64 a) { (void)f; (void)a; g_abort(AB_UNSUPPORTED); return R{0, true}; }
__device__ inline void apply_spawn(u64 f, u64 a, u64 parent) { (void)f; (void)a; (void)parent; g_abort(AB_UNSUPPORTED); }
__device__ inline u64 build_closure(u16 id, const u64 *caps, int n) { (void)id; (void)caps; (void)n; g_abort(AB_UNSUPPORTED); return 0; }

// share a value: O(1) refcount bump on the root cell
__device__ u64 dup_val(u64 p) {
  u64 t = tag(p);
  if (t >= TU) return p;
  if (t == T_LAM) return dup_closure(p);
  if (t == T_FLO) { rc_inc((u32)(p & M56)); return p; }
  if (t == T_CON) {
    if (con_ar(p) > 0) rc_inc(con_addr(p));
    return p;
  }
  if (t == T_ARR) { atomicAdd((unsigned long long *)arr_block(p), 1ull); return p; }
  return p;
}
// drop a reference; the last one tears the value down
__device__ inline void free_val(u64 p) {
  u64 t = tag(p);
  if (t != T_CON && t != T_FLO && t != T_ARR && t != T_LAM) return;
  free_val_slow(p);
}
__device__ void free_val_slow(u64 p) {
  u64 t = tag(p);
  if (t == T_ARR) { arr_drop(p); return; }
  if (t == T_LAM) { drop_closure(p); return; }
  if (t >= TU) return;
  if (t == T_FLO) {
    u32 a = (u32)(p & M56);
    if (rc_dec(a)) free_node(a);
    return;
  }
  if (t != T_CON || con_ar(p) == 0) return;
  u32 root = con_addr(p);
  if (!lin(con_tag(p)) && !rc_dec(root)) return;
  u64 q = p;
  for (;;) {
    u8 ar = con_ar(q);
    u32 ca = con_addr(q);
    u64 c0 = cell0(ca), c1 = cell1(ca);
    free_node(ca);
    if (ar > 2) {
      free_val_slow(c0);
      q = c1;
    } else {
      if (ar >= 1) free_val(c0);
      if (ar >= 2) free_val(c1);
      return;
    }
  }
}

// dynamic arithmetic (ints, or boxed floats)
__device__ u64 bin(u8 op, u64 a, u64 b, u8 own) {
  if (tag(a) == T_NUM && tag(b) == T_NUM) {
    i64 x = as_i(a), y = as_i(b), r = 0;
    switch (op) {
    case 0: r = (i64)((u64)x + (u64)y); break;
    case 1: r = (i64)((u64)x - (u64)y); break;
    case 2: r = (i64)((u64)x * (u64)y); break;
    case 3: r = idiv(x, y); break;
    case 4: r = floor_div(x, y); break;
    case 5: r = py_mod(x, y); break;
    case 6: r = (i64)((u64)x << ((u32)y & 63u)); break;
    case 7: r = x >> ((u32)y & 63u); break;
    case 8: r = x & y; break;
    case 9: r = x | y; break;
    default: r = x ^ y; break;
    }
    return num(wrap56(r));
  }
  double x = flo_val(a), y = flo_val(b), r = 0.0;
  if (own & 1) free_val(a);
  if (own & 2) free_val(b);
  switch (op) {
  case 0: r = __dadd_rn(x, y); break;
  case 1: r = __dsub_rn(x, y); break;
  case 2: r = __dmul_rn(x, y); break;
  case 3: r = __ddiv_rn(x, y); break;
  default: g_abort(AB_UNREACHABLE); break;
  }
  return flo(r);
}
__device__ u64 cmp(u8 op, u64 a, u64 b, u8 own) {
  bool r = false;
  if (tag(a) == T_NUM && tag(b) == T_NUM) {
    i64 x = as_i(a), y = as_i(b);
    switch (op) {
    case 0: r = x < y; break;
    case 1: r = x <= y; break;
    case 2: r = x > y; break;
    case 3: r = x >= y; break;
    case 4: r = x == y; break;
    default: r = x != y; break;
    }
  } else {
    double x = flo_val(a), y = flo_val(b);
    if (own & 1) free_val(a);
    if (own & 2) free_val(b);
    switch (op) {
    case 0: r = x < y; break;
    case 1: r = x <= y; break;
    case 2: r = x > y; break;
    case 3: r = x >= y; break;
    case 4: r = x == y; break;
    default: r = x != y; break;
    }
  }
  return num(r ? 1 : 0);
}
// n-tuple of int zeros (the identity of the additive fold combiners)
__device__ u64 zeros(usize n) {
  u64 fs[16];
  for (usize k = 0; k < n && k < 16; k++) fs[k] = num(0);
  return mk_con(0xfff, fs, (int)(n < 16 ? n : 16));
}
// elementwise wrapping add of two equal-shape int tuples, in place into a
__device__ u64 tup_add(u64 a, u64 b, bool mask32) {
  u64 pa = a, pb = b;
  for (;;) {
    u8 n = con_ar(pa);
    if (n == 0) return a;
    u32 ca = con_addr(pa), cb = con_addr(pb);
    u64 x0 = cell0(ca), y0 = cell0(cb);
    i64 s = (i64)((u64)as_i(x0) + (u64)as_i(y0));
    cell_set(ca, 0, num(mask32 ? (s & 0xffffffffll) : wrap56(s)));
    if (n > 2) {
      pa = cell1(ca);
      pb = cell1(cb);
      free_node(cb);
    } else {
      if (n == 2) {
        u64 x1 = cell1(ca), y1 = cell1(cb);
        i64 s1 = (i64)((u64)as_i(x1) + (u64)as_i(y1));
        cell_set(ca, 1, num(mask32 ? (s1 & 0xffffffffll) : wrap56(s1)));
      }
      free_node(cb);
      return a;
    }
  }
}

// ---- kernels (host wave loop launches these via the driver API) ----

extern "C" __global__ void k_boot(u64 a, u64 b, u64 c) { prog_fire(0, a, b, c); }

extern "C" __global__ void k_fire(u32 rule, u32 start, u32 count) {
  u32 i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i >= count)
    return;
  if (*G.abortf >= AB_ARENA)
    return; // poisoned: stop generating work
  const u64 *e = &G.ebuf[((u64)rule * G.bcap + start + i) * 3];
  prog_fire(rule, e[0], e[1], e[2]);
}

// Sequential-tail pump: when total pending work is tiny, one host wave per
// rewrite (sync + memcpy + launch) dominates, so the host launches this
// single-thread kernel instead. It drains entries one by one for up to
// max_steps rewrites, recycling fully drained buckets, and hands back to
// the parallel loop as soon as the frontier widens again.
extern "C" __global__ void k_pump(u32 max_steps) {
  u32 r0 = 0; // round-robin start (chains hop between few rules)
  for (u32 s = 0; s < max_steps; s++) {
    if (*G.abortf >= AB_ARENA)
      return;
    if ((s & 255u) == 255u) {
      u64 tot = 0;
      for (u32 r = 0; r < G.nrules; r++) {
        u32 len = G.blen[r] > G.bcap ? G.bcap : G.blen[r];
        tot += len - G.bdone[r];
      }
      if (tot > 1024)
        return; // wide again: let the host drain it in parallel
    }
    u32 rule = G.nrules;
    for (u32 k = 0; k < G.nrules; k++) {
      u32 r = (r0 + k) % G.nrules;
      u32 len = G.blen[r] > G.bcap ? G.bcap : G.blen[r];
      u32 d = G.bdone[r];
      if (d >= len) {
        if (len && d >= G.blen[r]) { // fully drained: recycle the bucket
          G.blen[r] = 0;
          G.bdone[r] = 0;
        }
        continue;
      }
      rule = r;
      break;
    }
    if (rule == G.nrules)
      return; // quiescent
    r0 = rule;
    u32 i = G.bdone[rule]++;
    const u64 *e = &G.ebuf[((u64)rule * G.bcap + i) * 3];
    prog_fire(rule, e[0], e[1], e[2]);
  }
}
