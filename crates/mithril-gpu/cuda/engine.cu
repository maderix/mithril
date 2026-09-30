// mithril-gpu: the device runtime (program-independent).
//
// A generated program.cu does:
//     #define PROG_NRULES <n>
//     #include "engine.cu"
//     ... the program's tables (LIN, LIN_TUP, UNBOX_CID), its functions,
//     __device__ R prog_dive(...) and __device__ void prog_fire(...)
// This file supplies the same helper vocabulary the lowered IR (lir) names
// on the CPU (`mithril_rt::prelude`): cells, refcounts, records, delivery,
// dives, constructors, sharing, arrays, arithmetic; plus the driver
// (`k_run`). Protocol mirrors the CPU engine (mithril-rt):
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

// abort codes (host maps each to a message). The first abort wins: every
// later one is a consequence (lanes run on to their next check), so the
// first is the root cause.
#define AB_UNREACHABLE 1u
#define AB_ARENA 2u
#define AB_OOB 3u
#define AB_UNSUPPORTED 4u
#define AB_LOOP 5u
#define AB_RECS 6u
#define AB_BUCKET 7u
#define AB_HEAP 8u
#define AB_ROUNDS 10u // the round limit (MITHRIL_GPU_ROUNDS): a run that does not converge
#define AB_DEEP 11u   // a native frame past the thread's stack (recursion too deep for the device)
#define AB_TIMEOUT 12u // written by the host at MITHRIL_GPU_TIMEOUT: stop
// a walk over cells that never ends is a corrupted arena, not a hang
#define GUARD(n) do { if (++(n) > (1u << 22)) { g_abort(AB_LOOP); return; } } while (0)
#define GUARDV(n, v) do { if (++(n) > (1u << 22)) { g_abort(AB_LOOP); return v; } } while (0)

struct Rec {
  int pend;
  unsigned short rule;
  unsigned short par; // 1: created in a GROW wave (its children run on different lanes)
  u32 d; // a spill chain head, a FILL target's low word, a TRMC head cell
  u32 s; // a FILL target's high word, a TRMC hole cell
  u64 parent; // rec_idx << 3 | slot
  u64 args[2];
};

struct Dev {
  u64 *nodes;  // ncap cell pairs
  u32 *rc;     // ncap refcounts (only read for non-linear constructors)
  Rec *recs;   // rcap records (0 = ROOT sink)
  u32 *nbump;  // cell bump (starts at 1: cell 0 reserved)
  u32 *rbump;  // record bump (starts at 1: rec 0 = ROOT)
  u32 *nfreen; // MAXLANES free-list heads (index+1)
  u32 *nchunk; // MAXLANES * 2 (cur, end) bump chunks
  u64 *ebuf;   // PROG_NRULES buckets of bcap 3-word entries
  u32 *blen;   // PROG_NRULES append counters (host drains by prefix)
  u32 *bdone;  // PROG_NRULES drained prefixes (shared with the host; a
               // fully drained bucket is recycled to 0 between waves)
  u64 *result; // [0] = delivered flag, [1] = ROOT value
  u32 *abortf;
  u64 *heap;   // array blocks: [rc, len|cls|flags, elems..] (bump chunks, per-lane free lists)
  u64 *hbump;
  u64 hcap;
  u64 *nw;     // MAXLANES * NWCAP * 2: per-lane net worklists (redex pairs)
  u32 *nwn;    // MAXLANES worklist lengths
  u32 *labels; // Dup label supply
  u32 *rfreen; // MAXLANES record free-list heads (index+1)
  u64 *lstk;   // MAXLANES * LSCAP lane-local tasks (rule, a, b, c), WORK phase
  u32 *lsn;    // MAXLANES lane-local task counts
  u32 ncap, rcap, bcap, chunksz, nrules;
  int fuel;    // per-dive budget
  int net_fuel; // rewrites per net reduction before spilling to the net rule
};
#define NWCAP 64
#define LSCAP 64

extern "C" {
// the runtime descriptor is written once by the host: constant memory, so
// a field read is a cached broadcast, not a dependent global load
__constant__ Dev G;
__device__ u64 g_rounds[8];            // rounds, grow sweeps, work phases, widest frontier, grow cycles, work cycles
// The rule table plus one rule the engine owns: ERA, the erasure of a
// value (the net's ERA-CON rewrites). A large teardown spills its subtrees
// as ERA tasks, so every lane erases a share instead of one lane walking
// the whole value.
#define ERA_RULE PROG_NRULES
#define NRULES_ALL (PROG_NRULES + 1)
#define ERA_CHUNK 256
#define ERA_DEPTH 32
// per thread: nodes erased by the current top-level teardown, its depth,
// and the work charged since the last forced suspension
__shared__ u32 s_era[256];
__shared__ u32 s_era_depth[256];
__shared__ u32 s_work[256];
__device__ u32 g_nrules = NRULES_ALL;
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

// per thread (blocks are at most 256 threads): 1 = the lane's own depth-
// first work, 0 = the parallel world (a GROW wave, or a cross-lane join a
// WORK lane is running at once): fork sites fork, spawns go global
__shared__ int s_mode[256];
#define WORK_MODE (s_mode[threadIdx.x])
// a lane-local task whose rule word carries this bit runs in the parallel world
#define PAR_TASK 0x80000000u
// the dive budget of the phase (a task's own body; a fork site's callee gets
// none in the parallel world)
__shared__ int s_fuel;

__device__ void prog_fire(u32 rule, u64 e0, u64 e1, u64 e2);
// rule -> its entries carry a record index in their third word
__device__ bool prog_rec_rule(u32 rule);
// rule -> its tasks can fork (the driver grows the frontier through these)
__device__ bool prog_forks(u32 rule);
__device__ inline bool rule_forks(u32 r) { return r == ERA_RULE || prog_forks(r); }
__device__ R prog_dive(u32 f, const u64 *args, i64 *fuel);
__device__ bool lin(u16 k);
__device__ u32 unbox_cid(u64 slot);

__device__ inline void g_abort(u32 code) { atomicCAS(G.abortf, 0u, code); }

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
// The stack guard: every dive form and non-leaf native function checks its
// frame against the thread's stack (the pointer at kernel entry, the limit
// from the runner) and aborts with AB_DEEP instead of faulting.
__shared__ unsigned long long s_sp0[256];
__device__ u32 g_stack_limit = 4 * 1024; // set by the runner before any launch
__device__ __forceinline__ unsigned long long sp_now() {
  unsigned long long v;
  asm volatile("stacksave.u64 %0;" : "=l"(v));
  return v;
}
__device__ inline void stack_mark() {
  s_sp0[threadIdx.x] = sp_now();
  s_era[threadIdx.x] = s_era_depth[threadIdx.x] = 0;
  s_work[threadIdx.x] = 0;
}
// Work charged on the device: it deepens no stack, so it does not touch
// the depth budget, but every WORK_CAP units it forces the frame's next
// budget check to suspend, so a dive form yields (a native loop checks
// nothing and runs to its end: design.md section 5.5).
#define WORK_CAP (1u << 20)
__device__ inline void work_fuel(i64 *fuel, i64 n) {
  u32 w = s_work[threadIdx.x] + (u32)n;
  if (w >= WORK_CAP) {
    w = 0;
    *fuel = -1;
  }
  s_work[threadIdx.x] = w;
}
__device__ inline bool stack_deep() {
  if (s_sp0[threadIdx.x] - sp_now() <= (unsigned long long)g_stack_limit) return false;
  g_abort(AB_DEEP);
  return true;
}


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
__device__ inline i64 f32_le(i64 a, i64 b) { return f32b(a) <= f32b(b) ? 1 : 0; }
__device__ inline i64 f32_from_u32(i64 a) { return f32i(__uint2float_rn((u32)a)); }
__device__ inline i64 f32_to_u32(i64 a) {
  float x = f32b(a);
  if (x != x || x < 0.0f || x >= 4294967296.0f) return 0;
  return (i64)(u32)x;
}

// ---- clamped cell access (safe even on garbage after an abort) ----

#include <cstdio>
#include <cooperative_groups.h>
namespace cg = cooperative_groups;
__device__ inline void expect_con(u64 p, const char *site) {
  if (tag(p) != T_CON) { if (atomicCAS(G.abortf, 0u, 9u) == 0) printf("mithril-gpu: %s on a non-constructor port %llx\n", site, p); }
}
__device__ inline u32 nclamp(u32 i) {
  if (i >= G.ncap) {
    // a cell index outside the arena is a corrupted port, never a valid read
    if (atomicCAS(G.abortf, 0u, 9u) == 0) printf("mithril-gpu: bad cell index %u\n", i);
    return G.ncap - 1;
  }
  return i;
}
__device__ inline u64 cell0(u32 i) { return G.nodes[2 * (u64)nclamp(i)]; }
__device__ inline u64 cell1(u32 i) { return G.nodes[2 * (u64)nclamp(i) + 1]; }
__device__ inline void setcell(u32 i, u64 a, u64 b) {
  u32 j = nclamp(i);
  G.nodes[2 * (u64)j] = a;
  G.nodes[2 * (u64)j + 1] = b;
}
__device__ inline void cell_set(u32 i, usize slot, u64 v) { G.nodes[2 * (u64)nclamp(i) + slot] = v; }

// ---- node arena: lane free list -> global overflow ring -> checked bump ----

// Cells: a per-lane intrusive free list (the link in the cell's first
// word, the head in `nfreen`, index+1, 0 = empty: unbounded, no atomics)
// and a bump of chunks per lane from the global counter. A lane's chunks
// start at 64 cells and double up to `chunksz`: the arena is managed
// memory committed on first touch, so the cells a program touches, not the
// lane count, decide the memory it commits.
__device__ u8 g_nclog[MAXLANES]; // log2 of the lane's next chunk size, minus 6
__device__ __forceinline__ u32 alloc_node(u64 a, u64 b) {
  u32 L = lane();
  u32 h = G.nfreen[L];
  if (h) {
    u32 i = h - 1;
    G.nfreen[L] = (u32)G.nodes[2 * (u64)i];
    setcell(i, a, b);
    return i;
  }
  u32 *ck = &G.nchunk[2 * L];
  if (ck[0] >= ck[1]) {
    u32 lg = g_nclog[L];
    u32 sz = min(64u << lg, G.chunksz);
    if ((64u << lg) < G.chunksz) g_nclog[L] = lg + 1;
    u32 base = atomicAdd(G.nbump, sz);
    if (base >= G.ncap) {
      g_abort(AB_ARENA);
      return 0;
    }
    ck[0] = base;
    u64 end = (u64)base + sz;
    ck[1] = end > (u64)G.ncap ? G.ncap : (u32)end;
  }
  u32 i = ck[0]++;
  setcell(i, a, b);
  return i;
}
__device__ __forceinline__ void free_node(u32 i) {
  if (i == 0 || i >= G.ncap)
    return;
  u32 L = lane();
  G.nodes[2 * (u64)i] = G.nfreen[L];
  G.nfreen[L] = i + 1;
}

__device__ inline u32 alloc2(u64 a, u64 b) { u32 i = alloc_node(a, b); G.rc[nclamp(i)] = 1; return i; }
__device__ inline void cell_free(u32 i) { free_node(i); }
__device__ inline void tok_free(u32 tok) { if (tok != NOTOK) free_node(tok); }
__device__ inline void rc_inc(u32 i) { atomicAdd(&G.rc[nclamp(i)], 1u); }
__device__ inline bool rc_dec(u32 i) { return atomicSub(&G.rc[nclamp(i)], 1u) == 1u; }
__device__ inline void rc_set1(u32 i) { G.rc[nclamp(i)] = 1; }
// refcounts are shared across SMs within a wave: read them past L1
__device__ inline bool rc_unique(u32 i) { return *(volatile u32 *)&G.rc[nclamp(i)] == 1; }

// ---- records / buckets / delivery ----

// Records: the same per-lane intrusive free list (the link in `d`, the
// head in `rfreen`) and a global bump.
__device__ inline void rec_free(u32 i) {
  if (i == 0 || i >= G.rcap) return;
  u32 L = lane();
  G.recs[i].d = G.rfreen[L];
  G.rfreen[L] = i + 1;
}
__device__ __forceinline__ u32 alloc_rec(u16 rule, u32 pend, u32 d, u32 s, u64 parent) {
  u32 L = lane();
  u32 i;
  u32 h = G.rfreen[L];
  if (h) {
    i = h - 1;
    G.rfreen[L] = G.recs[i].d;
  } else {
    i = atomicAdd(G.rbump, 1);
    if (i >= G.rcap) {
      g_abort(AB_RECS);
      return G.rcap - 1;
    }
  }
  Rec &r = G.recs[i];
  r.pend = (int)pend;
  r.rule = (unsigned short)rule;
  r.par = WORK_MODE ? 0 : 1;
  r.s = s;
  r.d = d;
  r.parent = parent;
  r.args[0] = 0;
  r.args[1] = 0;
  return i;
}

__device__ inline u32 rclamp(u32 i) { return i < G.rcap ? i : G.rcap - 1; }
__device__ inline u64 rec_parent(u32 rec) { return G.recs[rclamp(rec)].parent; }
__device__ inline u32 rec_d(u32 rec) { return G.recs[rclamp(rec)].d; }
__device__ inline u32 rec_s(u32 rec) { return G.recs[rclamp(rec)].s; }
__device__ inline void set_parent(u32 rec, u64 parent) { G.recs[rclamp(rec)].parent = parent; }
__device__ inline i64 fuel_of() { return (i64)s_fuel; } // the phase's budget (fold estimates)

// Buckets are rings: `blen` and `bdone` grow without bound (bcap is a
// power of two, so u32 wrap-around keeps `i % bcap` consistent); only the
// live entries, blen - bdone, are capped.

__device__ __noinline__ void spawn_global(u32 rule, u64 a, u64 b, u64 c);
__device__ __noinline__ void spawn3(u32 rule, u64 a, u64 b, u64 c) {
  if (WORK_MODE) {
    u32 L = lane();
    u32 n = G.lsn[L];
    if (n < LSCAP) {
      u64 *t = &G.lstk[((u64)L * LSCAP + n) * 4];
      t[0] = rule;
      t[1] = a;
      t[2] = b;
      t[3] = c;
      G.lsn[L] = n + 1;
      return;
    }
  }
  spawn_global(rule, a, b, c);
}
__device__ __noinline__ void spawn_global(u32 rule, u64 a, u64 b, u64 c) {
  u32 i = atomicAdd(&G.blen[rule], 1);
  if (i - *(volatile u32 *)&G.bdone[rule] >= G.bcap) {
    g_abort(AB_BUCKET);
    return;
  }
  u64 *e = &G.ebuf[((u64)rule * G.bcap + (i & (G.bcap - 1))) * 3];
  e[0] = a;
  e[1] = b;
  e[2] = c;
}

// A cross-lane join just completed. The CPU runtime's rule (reference's design too):
// the lane whose delivery completed it runs it at once, in the parallel
// world (what it forks goes to the global rings for the next GROW), in
// every kernel. It waits on the lane's own stack, not the C stack, and
// the kernel drains that stack after each task (`drain_local`).
__device__ inline void join_ready(u32 rule, u64 a, u64 b, u64 c) {
  {
    u32 L = lane();
    u32 n = G.lsn[L];
    if (n < LSCAP) {
      u64 *t = &G.lstk[((u64)L * LSCAP + n) * 4];
      t[0] = rule | PAR_TASK;
      t[1] = a;
      t[2] = b;
      t[3] = c;
      G.lsn[L] = n + 1;
      return;
    }
  }
  spawn_global(rule, a, b, c);
}
// A ready record (a task with no inputs: the rest of a body after a fork
// site) runs at once on this lane: in the parallel world the body then
// reaches all its fork sites in one step (adopted from reference's runtime
// design: a fork releases every child at once); in WORK it is the lane's own.
__device__ inline void ready_rec(u32 rec) {
  Rec &r = G.recs[rclamp(rec)];
  if (r.par)
    join_ready(r.rule, r.args[0], r.args[1], (u64)rec);
  else
    spawn3(r.rule, r.args[0], r.args[1], (u64)rec);
}

__device__ __noinline__ void deliver(u64 parent, u64 val) {
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
  if (atomicSub(&r.pend, 1) == 1) {
    // the other slot was written by another SM in this wave: read it past L1
    volatile u64 *args = (volatile u64 *)r.args;
    __threadfence();
    // a join created in a GROW wave has children on different lanes: its
    // continuation runs in the parallel world (the global rings, where the
    // next GROW can widen what it forks); a join created in WORK is the
    // lane's own and continues on it
    if (r.par)
      join_ready(r.rule, args[0], args[1], (u64)ri);
    else
      spawn3(r.rule, args[0], args[1], (u64)ri);
  }
}

// Spawn a saturated call: arity <= 2 rides in (a, b); wider calls put
// arg0 in `a` and chain args[1..] through cells in `b` (addr+1, 0 = end).
__device__ __noinline__ void spawn_call(u16 rule, const u64 *args, int n, u64 parent) {
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
  if (*ch == 0 || (*ch >> 32) != 0) { if (atomicCAS(G.abortf, 0u, 9u) == 0) printf("mithril-gpu: pop_chain on %llx\n", *ch); }
  u32 i = (u32)(*ch - 1);
  u64 v = cell0(i);
  *ch = cell1(i);
  free_node(i);
  return v;
}

// Dive `f` (args[0] = the destination) and deliver its result there; a
// suspended dive's residue root is attached to the destination.
__device__ __noinline__ void dive_to(u16 f, const u64 *args, int n) {
  i64 fuel = (i64)s_fuel;
  R r = prog_dive(f, args, &fuel);
  if (r.ok)
    deliver(args[0], r.v);
  else if (args[0] != NONE)
    set_parent((u32)r.v, args[0]);
}

// A segment's tail call delivering to args[0]: in the parallel world it
// becomes a task at once (it dives with no budget: the entry suspends and
// spawns the call against a forwarding record attached to the
// destination); in WORK it dives here.
__device__ __noinline__ void tail_to(u16 f, const u64 *args, int n) {
  if (WORK_MODE) {
    dive_to(f, args, n);
    return;
  }
  i64 zero = 0;
  R r = prog_dive(f, args, &zero);
  if (r.ok)
    deliver(args[0], r.v);
  else if (args[0] != NONE)
    set_parent((u32)r.v, args[0]);
}

// Dive `f` with no destination (args[0] = NONE): ok = the value, else the
// root record of its residue, whose parent the caller sets.
//
// The two worlds (adopted from reference's runtime design; design.md s14): in the sequential world (WORK) a callee gets
// the lane's budget and runs here; in the parallel world (a GROW sweep) a
// callee gets no budget, so it suspends at entry and becomes a task at
// once, and the caller captures its continuation as records. A task's own
// body runs between its calls; the frontier widens by one call level per
// sweep, evenly.
__device__ i64 g_fz[MAXLANES]; // the empty budget a callee gets in the parallel world
__device__ inline i64 *fork_fuel(i64 *fuel) {
  if (WORK_MODE) return fuel;
  u32 L = lane();
  g_fz[L] = 0;
  return &g_fz[L];
}
// a cut: the callee runs here with the lane's budget in both worlds
__device__ __noinline__ R dive_res(u16 f, const u64 *args, int n) {
  i64 fuel = (i64)s_fuel;
  return prog_dive(f, args, &fuel);
}
// a fork site's callee: a task at once in the parallel world
__device__ __noinline__ R dive_res_fork(u16 f, const u64 *args, int n) {
  i64 fuel = WORK_MODE ? (i64)s_fuel : 0;
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
__device__ __noinline__ u64 hole_wrap(u64 r, u64 head, u32 hole, u16 rule) {
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
__device__ __noinline__ u64 mk_con(u16 k, const u64 *fs, int n) {
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
__device__ __noinline__ u64 field(u64 p, usize i) {
  expect_con(p, "field");
  u32 g = 0;
  for (;;) {
    GUARDV(g, 0);
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
  expect_con(p, "consume2k");
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
  expect_con(p, "consume2r");
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
__device__ __noinline__ u64 take_field(u64 p, usize i) {
  expect_con(p, "take_field");
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
#define ARR_LEN_MASK ((1ull << 48) - 1)
#define ARR_CLS_SHIFT 48 // the block's size class (a power of two of words) in word 1
// Freed blocks go on the lane's intrusive free list of their size class (the
// link in word 0, the head index+1): the cells' access model, no atomics.
// Eight classes per octave: a block is at most 1/8 larger than its array.
#define ACLS 256
__device__ u32 g_afree[MAXLANES * ACLS];
__device__ inline u64 arr_cls_words(u32 c) { return c < 8 ? 8 : (8ull + (c & 7)) << (c / 8 - 3); }
__device__ inline u32 arr_cls(usize n) {
  u64 w = n + 2;
  if (w <= 8) return 0;
  u32 e = 63 - __clzll(w - 1);              // 2^e <= w-1
  u64 step = 1ull << (e - 3);               // the octave [2^e, 2^(e+1)] in eighths
  u32 sub = (u32)((w - (1ull << e) + step - 1) / step); // 1..8
  return sub == 8 ? 8 * (e + 1) : 8 * e + sub;
}
__device__ inline u64 *arr_block(u64 p) { return &G.heap[p & M56]; }
__device__ inline usize arr_len_of(u64 p) { return arr_block(p)[1] & ARR_LEN_MASK; }
__device__ inline u64 *arr_elems(u64 p) { return arr_block(p) + 2; }
__device__ inline bool arr_raw(u64 p) { return (arr_block(p)[1] & ARR_RAW) != 0; }
__device__ inline bool arr_boxed(u64 p) { return (arr_block(p)[1] & ARR_BOXED) != 0; }
__device__ inline bool is_heap(u64 v) { u64 t = tag(v); return t == T_CON || t == T_FLO || t == T_ARR || t == T_LAM; }
__device__ inline void arr_mark_boxed(u64 a, u64 v) { if (is_heap(v)) arr_block(a)[1] |= ARR_BOXED; }
__device__ inline u64 arr_elem(u64 p, usize k) { u64 e = arr_elems(p)[k]; return arr_raw(p) ? retag((i64)e) : e; }
__device__ __noinline__ u64 arr_alloc_fill(usize n, u64 fill) {
  if ((u64)n + 2 > G.hcap) { // before the class: a huge n has no class
    g_abort(AB_HEAP);
    return (T_ARR << 56) | 0;
  }
  u32 cls = arr_cls(n);
  u32 *h = &g_afree[lane() * ACLS + cls];
  u64 base;
  if (*h) {
    base = *h - 1;
    *h = (u32)G.heap[base];
  } else {
    u64 words = arr_cls_words(cls);
    base = atomicAdd(G.hbump, words);
    if (base + words > G.hcap) {
      g_abort(AB_HEAP);
      return (T_ARR << 56) | 0;
    }
  }
  u64 *b = &G.heap[base];
  b[0] = 1;
  b[1] = (u64)n | ((u64)cls << ARR_CLS_SHIFT);
  for (usize k = 0; k < n; k++) b[2 + k] = fill;
  return (T_ARR << 56) | base;
}
__device__ inline u64 arr_alloc(usize n) { return arr_alloc_fill(n, 0); }
__device__ inline void arr_free_block(u64 p) {
  u64 base = p & M56;
  u32 cls = (u32)(G.heap[base + 1] >> ARR_CLS_SHIFT) & 255;
  u32 *h = &g_afree[lane() * ACLS + cls];
  G.heap[base] = *h;
  *h = (u32)base + 1;
}
__device__ inline void arr_oob(i64 i, usize n) { (void)i; (void)n; g_abort(AB_OOB); }
__device__ inline u64 arr_new_raw(i64 n, i64 x) {
  if (n < 0) { g_abort(AB_OOB); n = 0; }
  u64 p = arr_alloc_fill((usize)n, (u64)x);
  if ((p & M56) != 0) arr_block(p)[1] |= ARR_RAW; // not the empty sentinel
  return p;
}
__device__ __noinline__ void arr_unraw(u64 a) {
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
__device__ __noinline__ u64 arr_copy(u64 a) {
  // the block's own length: an allocation that aborted returns the empty
  // block at heap word 0
  u64 b = arr_alloc(arr_len_of(a));
  usize n = arr_len_of(b);
  if (arr_boxed(a)) {
    for (usize k = 0; k < n; k++) arr_elems(b)[k] = dup_val(arr_elems(a)[k]);
    if ((b & M56) != 0) arr_block(b)[1] |= ARR_BOXED; // not the empty sentinel
  } else {
    for (usize k = 0; k < n; k++) arr_elems(b)[k] = arr_elems(a)[k];
    if (arr_raw(a) && (b & M56) != 0) arr_block(b)[1] |= ARR_RAW;
  }
  free_val(a);
  return b;
}
__device__ inline u64 arr_own(u64 a) { return arr_rc_load(a) == 1 ? a : arr_copy(a); }
__device__ __noinline__ u64 arr_new(i64 n, u64 v) {
  if (n < 0) { g_abort(AB_OOB); n = 0; }
  if (tag(v) == T_NUM) return arr_new_raw(n, sh(v));
  usize un = (usize)n;
  if (!is_heap(v)) return arr_alloc_fill(un, v);
  u64 p = arr_alloc(un);
  un = arr_len_of(p); // 0 when the allocation aborted
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
__device__ __noinline__ u64 arr_set(u64 a, i64 i, u64 v) {
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
// An ERA continuation owns the zero-reference array until its suffix is erased.
// e1 encodes next index + 1; ordinary ERA tasks keep e1 == 0.
__device__ void arr_erase(u64 p, usize start) {
  if (s_era_depth[threadIdx.x]++ == 0) s_era[threadIdx.x] = 0;
  struct Leave { __device__ ~Leave() { s_era_depth[threadIdx.x]--; } } leave;
  usize n = arr_len_of(p);
  for (usize k = start; k < n; k++) {
    if (s_era[threadIdx.x] >= ERA_CHUNK || s_era_depth[threadIdx.x] > ERA_DEPTH) {
      spawn_global(ERA_RULE, p, k + 1, 0);
      return;
    }
    ++s_era[threadIdx.x];
    free_val(arr_elems(p)[k]);
  }
  arr_free_block(p);
}
__device__ __noinline__ void arr_drop(u64 p) {
  if (atomicAdd((unsigned long long *)arr_block(p), 0xffffffffffffffffull) != 1ull) return;
  if (arr_boxed(p)) arr_erase(p, 0); else arr_free_block(p);
}

// ---- closures: the net region (below, after the value helpers) ----

__device__ u64 dup_closure(u64 p);
__device__ void drop_closure(u64 p);
__device__ R apply(u64 f, u64 a);
// settling a net value compiled code will read (defined with the bridge)
// no record yet (record 0 is the root sink, never allocated)
#define NOREC 0u
__device__ int settle_await(u64 v, u16 rule, u32 d, u32 s, u64 parent, u32 *j);
__device__ bool settle_release(u32 j);
__device__ bool settled(u64 v, u16 rule, u32 d, u32 s, u64 parent, u32 *j);
__device__ void apply_spawn(u64 f, u64 a, u64 parent);
__device__ u64 build_closure(u16 id, const u64 *caps, int n);

// share a value: O(1) refcount bump on the root cell
__device__ __forceinline__ u64 dup_val(u64 p) {
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
__device__ __forceinline__ void free_val_slow(u64 p) {
  u32 g = 0;
  if (s_era_depth[threadIdx.x]++ == 0) s_era[threadIdx.x] = 0;
  struct Leave { __device__ ~Leave() { s_era_depth[threadIdx.x]--; } } leave;
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
    GUARD(g);
    u8 ar = con_ar(q);
    u32 ca = con_addr(q);
    u64 c0 = cell0(ca), c1 = cell1(ca);
    free_node(ca);
    // past the node budget, or ERA_DEPTH frames down (the frames stack on
    // the caller's), the rest is ERA tasks for every lane
    bool spill = ++s_era[threadIdx.x] > ERA_CHUNK || s_era_depth[threadIdx.x] > ERA_DEPTH;
    if (ar > 2) {
      if (spill && tag(c0) == T_CON && con_ar(c0) > 0) spawn_global(ERA_RULE, c0, 0, 0); else free_val_slow(c0);
      q = c1;
    } else {
      if (ar >= 1) { if (spill && tag(c0) == T_CON && con_ar(c0) > 0) spawn_global(ERA_RULE, c0, 0, 0); else free_val(c0); }
      if (ar >= 2) { if (spill && tag(c1) == T_CON && con_ar(c1) > 0) spawn_global(ERA_RULE, c1, 0, 0); else free_val(c1); }
      return;
    }
  }
}

// dynamic arithmetic (ints, or boxed floats)
__device__ __noinline__ u64 bin(u8 op, u64 a, u64 b, u8 own) {
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
__device__ __noinline__ u64 cmp(u8 op, u64 a, u64 b, u8 own) {
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
__device__ __noinline__ u64 zeros(usize n) {
  u64 fs[16];
  for (usize k = 0; k < n && k < 16; k++) fs[k] = num(0);
  return mk_con(0xfff, fs, (int)(n < 16 ? n : 16));
}
// elementwise wrapping add of two equal-shape int tuples, in place into a
__device__ __noinline__ u64 tup_add(u64 a, u64 b, bool mask32) {
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

// ======================================================================
// The net region: the interaction rule table (mithril_core::rules) and
// the instantiation of entries, on the device arena. Same rules, same
// encodings as the CPU; `process` is the reference's shape line by line.
// The program supplies NFNS, NET_RULE, FILL_RULE, FWD_RULE, prog_inst
// (entry bodies as net builders) and the match tables.

#define T_VAR 0ull
#define T_ERA 1ull
#define T_DUP 5ull
#define T_APP 7ull
#define T_OP 8ull
#define T_SWI 9ull
#define T_MAT 10ull
#define T_REF 11ull
#define T_EXT 12ull
#define T_KONT 13ull
#define EMPTY ((T_EXT << 56) | M56)
#define OP_FLIP 0x100u
#define CTAG_TUPLE 0xfffu
#define ARR_PAIR 45u
#define LISTCAP 64

__device__ inline u64 payload(u64 p) { return p & M56; }
__device__ inline u64 mkport(u64 t, u64 pl) { return (t << 56) | (pl & M56); }
__device__ inline u64 era() { return mkport(T_ERA, 0); }
__device__ inline u64 op_port(u32 addr, u16 code) { return mkport(T_OP, ((u64)addr << 16) | code); }
__device__ inline u32 op_addr(u64 p) { return (u32)(payload(p) >> 16); }
__device__ inline u16 op_code(u64 p) { return (u16)(payload(p) & 0xffff); }
__device__ inline u64 dup_port(u32 addr, u32 label) { return mkport(T_DUP, ((u64)addr << 24) | (label & 0xffffffu)); }
__device__ inline u32 dup_addr(u64 p) { return (u32)(payload(p) >> 24); }
__device__ inline u32 dup_label(u64 p) { return (u32)(payload(p) & 0xffffffu); }
__device__ inline u64 mat_port(u32 addr, u16 id) { return mkport(T_MAT, ((u64)addr << 16) | id); }
__device__ inline u32 mat_addr(u64 p) { return (u32)(payload(p) >> 16); }
__device__ inline u16 mat_id(u64 p) { return (u16)(payload(p) & 0xffff); }
__device__ inline u64 ref_port(u64 head, u16 entry) {
  u64 h = head == EMPTY ? 0 : payload(head) + 1;
  return mkport(T_REF, (h << 16) | entry);
}
__device__ inline u64 ref_head(u64 p) {
  u64 h = payload(p) >> 16;
  return h == 0 ? EMPTY : mkport(T_EXT, h - 1);
}
__device__ inline u16 ref_entry(u64 p) { return (u16)(payload(p) & 0xffff); }
__device__ inline u64 kont_port(u64 parent) { return mkport(T_KONT, parent); }

__device__ inline u32 fresh_label() {
  u32 l = atomicAdd(G.labels, 1u) & 0xffffffu;
  return l == 0 ? 1 : l;
}
__device__ inline u64 wire() { return mkport(T_VAR, alloc_node(EMPTY, EMPTY)); }
__device__ inline u64 alloc_flo(double f) { return mkport(T_FLO, alloc2(__double_as_longlong(f), 0)); }

// the lane's worklist of generic redexes; overflow spills to the net rule
__device__ inline void push_redex(u64 a, u64 b) {
  u32 L = lane();
  u32 n = G.nwn[L];
  if (n < NWCAP) {
    G.nw[(L * NWCAP + n) * 2] = a;
    G.nw[(L * NWCAP + n) * 2 + 1] = b;
    G.nwn[L] = n + 1;
  } else {
    spawn3(NET_RULE, a, b, 0);
  }
}
__device__ inline bool pop_redex(u64 *a, u64 *b) {
  u32 L = lane();
  u32 n = G.nwn[L];
  if (n == 0) return false;
  n -= 1;
  *a = G.nw[(L * NWCAP + n) * 2];
  *b = G.nw[(L * NWCAP + n) * 2 + 1];
  G.nwn[L] = n;
  return true;
}

// ---- list chains ([item, Ext(next)|EMPTY] cells) and constructor chains ----

__device__ __noinline__ u64 list_alloc(const u64 *items, int n) {
  u64 head = EMPTY;
  for (int i = n - 1; i >= 0; i--) head = mkport(T_EXT, alloc_node(items[i], head));
  return head;
}
// walk and free a list chain into buf (at most LISTCAP items)
__device__ __noinline__ int list_collect(u64 head, u64 *buf) {
  int n = 0;
  u32 g = 0;
  while (head != EMPTY) {
    GUARDV(g, n);
    u32 a = (u32)payload(head);
    u64 c0 = cell0(a), c1 = cell1(a);
    free_node(a);
    if (n < LISTCAP) buf[n] = c0;
    n++;
    head = c1;
  }
  if (n > LISTCAP) { g_abort(AB_ARENA); return LISTCAP; }
  return n;
}
__device__ __noinline__ u64 con_alloc(u16 ctag, const u64 *fields, int n) {
  if (n == 0) return con(0, ctag, 0);
  if (n == 1) return con(alloc2(fields[0], EMPTY), ctag, 1);
  if (n == 2) return con(alloc2(fields[0], fields[1]), ctag, 2);
  u64 rest = con_alloc(ctag, fields + 1, n - 1);
  return con(alloc2(fields[0], rest), ctag, (u8)(n < 15 ? n : 15));
}
// walk and free a constructor chain into buf
__device__ __noinline__ int con_collect(u64 p, u64 *buf) {
  int n = 0;
  for (;;) {
    u8 ar = con_ar(p);
    u32 a = con_addr(p);
    if (ar == 0) return n;
    u64 c0 = cell0(a), c1 = cell1(a);
    free_node(a);
    if (ar == 1) { buf[n++] = c0; return n; }
    if (ar == 2) { buf[n++] = c0; buf[n++] = c1; return n; }
    buf[n++] = c0;
    if (n >= LISTCAP - 2) { g_abort(AB_ARENA); return n; }
    p = c1;
  }
}

// ---- wiring ----

// Connect two ports. Wire cells hold the first arrival in slot 0; the
// second arrival takes it (freeing the cell) and the two ports meet.
__device__ __noinline__ void link(u64 a, u64 b) {
  u32 g = 0;
  for (;;) {
    GUARD(g);
    if (tag(a) != T_VAR) {
      if (tag(b) != T_VAR) { push_redex(a, b); return; }
      u64 t = a; a = b; b = t;
    }
    u32 w = (u32)payload(a);
    u64 c0 = cell0(w);
    if (c0 == EMPTY) { cell_set(w, 0, b); return; }
    free_node(w);
    a = b;
    b = c0;
  }
}
// Follow filled wires (freeing them) until a non-Var port or an unfilled
// wire end is reached.
__device__ __noinline__ u64 resolve(u64 p) {
  u32 g = 0;
  while (tag(p) == T_VAR) {
    GUARDV(g, p);
    u32 w = (u32)payload(p);
    u64 c0 = cell0(w);
    if (c0 == EMPTY) return p;
    free_node(w);
    p = c0;
  }
  return p;
}

// ---- the program's half (prog_* supplied by program.cu) ----

__device__ void prog_inst(u16 entry, const u64 *args, int n, u64 ret);
__device__ bool prog_mat_proj(u16 mid, usize *i);
__device__ const u16 *prog_mat_arms(u16 mid, int *n);

__device__ inline bool is_ext_value(u64 p) { u64 t = tag(p); return t >= TU || t == T_ARR; }
__device__ inline bool is_value(u64 p) {
  u64 t = tag(p);
  return t == T_NUM || t == T_FLO || t == T_CON || t == T_LAM || is_ext_value(p);
}
__device__ inline bool is_closure(u64 r) { return ref_entry(r) >= NFNS; }

// REF rule: a compiled function with every argument produced runs by its
// CALL rule and comes back through a FILL record; anything else (a lifted
// branch/arm, a call met before its arguments) is instantiated as a net.
__device__ __noinline__ void unfold(u64 r, u64 other) {
  if (ref_entry(r) < NFNS) {
    // compiled code reads its arguments whole: a call whose arguments still
    // have pending fields waits for them, then meets its output again
    bool produced = true;
    u32 g = 0;
    for (u64 h = ref_head(r); h != EMPTY && produced; h = cell1((u32)payload(h))) {
      GUARD(g);
      produced = tag(cell0((u32)payload(h))) != T_VAR;
    }
    u32 j = NOREC;
    g = 0;
    for (u64 h = ref_head(r); h != EMPTY && produced; h = cell1((u32)payload(h))) {
      GUARD(g);
      u64 a = cell0((u32)payload(h));
      if (settle_await(a, RELINK_RULE, (u32)r, (u32)(r >> 32), other, &j) < 0) return;
    }
    if (j != NOREC && !settle_release(j)) return;
  }
  u64 args[LISTCAP];
  int n = list_collect(ref_head(r), args);
  u16 entry = ref_entry(r);
  bool produced = true;
  for (int i = 0; i < n; i++) if (tag(args[i]) == T_VAR) produced = false;
  if (entry < NFNS && produced) {
    u32 ri = alloc_rec(FILL_RULE, 1, (u32)other, (u32)(other >> 32), NONE);
    spawn_call((u16)(1 + entry), args, n, rec_addr(ri));
  } else {
    prog_inst(entry, args, n, other);
  }
}

// builtins on runtime values (codes as mithril_core::lower / net_compute)
__device__ __noinline__ bool net_compute(u16 code, u64 x, u64 y, u64 *out) {
  if (code >= 32) {
    switch (code) {
    case 32: *out = num(f32_add(as_i(x), as_i(y))); return true;
    case 33: *out = num(f32_sub(as_i(x), as_i(y))); return true;
    case 34: *out = num(f32_mul(as_i(x), as_i(y))); return true;
    case 35: *out = num(f32_div(as_i(x), as_i(y))); return true;
    case 36: *out = num(f32_sqrt(as_i(x))); return true;
    case 37: *out = num(f32_lt(as_i(x), as_i(y))); return true;
    case 38: *out = num(f32_from_u32(as_i(x))); return true;
    case 39: *out = num(f32_to_u32(as_i(x))); return true;
    case 40: *out = arr_new(as_i(x), y); return true;
    case 41: *out = arr_get(x, as_i(y)); return true;
    case 42: *out = num((i64)arr_len_of(x)); return true;
    case 43: {
      u64 i = field(y, 0), v = field(y, 1);
      free_node(con_addr(y));
      *out = arr_set(x, as_i(i), v);
      return true;
    }
    case 44: *out = num(f32_le(as_i(x), as_i(y))); return true;
    case 45: { u64 fs[2] = {x, y}; *out = mk_con(0xfff, fs, 2); return true; }
    default: g_abort(AB_UNREACHABLE); return false;
    }
  }
  if (tag(x) == T_NUM && tag(y) == T_NUM) {
    i64 a = as_i(x), b = as_i(y);
    if (code >= 16) {
      bool r = false;
      switch (code) {
      case 16: r = a < b; break;
      case 17: r = a <= b; break;
      case 18: r = a > b; break;
      case 19: r = a >= b; break;
      case 20: r = a == b; break;
      default: r = a != b; break;
      }
      *out = num(r ? 1 : 0);
      return true;
    }
    if ((code == 3 || code == 4 || code == 5) && b == 0) { g_abort(AB_OOB); return false; }
    i64 r = 0;
    switch (code) {
    case 0: r = (i64)((u64)a + (u64)b); break;
    case 1: r = (i64)((u64)a - (u64)b); break;
    case 2: r = (i64)((u64)a * (u64)b); break;
    case 3: r = idiv(a, b); break;
    case 4: r = floor_div(a, b); break;
    case 5: r = py_mod(a, b); break;
    case 6: r = (i64)((u64)a << ((u32)b & 63u)); break;
    case 7: r = a >> ((u32)b & 63u); break;
    case 8: r = a & b; break;
    case 9: r = a | b; break;
    default: r = a ^ b; break;
    }
    *out = num(wrap56(r));
    return true;
  }
  double a = flo_val(x), b = flo_val(y);
  free_node((u32)(x & M56));
  free_node((u32)(y & M56));
  if (code >= 16) {
    bool r = false;
    switch (code) {
    case 16: r = a < b; break;
    case 17: r = a <= b; break;
    case 18: r = a > b; break;
    case 19: r = a >= b; break;
    case 20: r = a == b; break;
    default: r = a != b; break;
    }
    *out = num(r ? 1 : 0);
    return true;
  }
  double r = 0.0;
  switch (code) {
  case 0: r = __dadd_rn(a, b); break;
  case 1: r = __dsub_rn(a, b); break;
  case 2: r = __dmul_rn(a, b); break;
  case 3: r = __ddiv_rn(a, b); break;
  default: g_abort(AB_UNREACHABLE); return false;
  }
  *out = flo(r);
  return true;
}

// ---- the rules ----

// ERA-anything: consume and erase the value/agent p
__device__ __noinline__ void era_value(u64 p) {
  u64 t = tag(p);
  if (t == T_ERA || t == T_NUM) return;
  if (t == T_FLO) { free_node((u32)payload(p)); return; }
  if (t == T_CON) {
    u64 fs[LISTCAP];
    int n = con_collect(p, fs);
    for (int i = 0; i < n; i++) link(era(), fs[i]);
    return;
  }
  if (t == T_DUP) {
    u32 d = dup_addr(p);
    u64 c0 = cell0(d), c1 = cell1(d);
    free_node(d);
    link(era(), c0);
    link(era(), c1);
    return;
  }
  if (t == T_LAM || t == T_APP) {
    u32 a = (u32)payload(p);
    u64 c0 = cell0(a), c1 = cell1(a);
    free_node(a);
    link(era(), c0);
    link(era(), c1);
    return;
  }
  if (t == T_OP) {
    u32 a = op_addr(p);
    u64 c0 = cell0(a), c1 = cell1(a);
    free_node(a);
    link(era(), c0);
    link(era(), c1);
    return;
  }
  if (t == T_SWI) {
    u32 s = (u32)payload(p);
    u64 c0 = cell0(s), c1 = cell1(s);
    free_node(s);
    link(era(), c0);
    u32 s2 = (u32)payload(c1);
    u64 d0 = cell0(s2), d1 = cell1(s2);
    free_node(s2);
    link(era(), d0);
    link(era(), d1);
    return;
  }
  if (t == T_MAT) {
    u32 m = mat_addr(p);
    u64 c0 = cell0(m), c1 = cell1(m);
    free_node(m);
    link(era(), c0);
    u64 rs[LISTCAP];
    int n = list_collect(c1, rs);
    for (int i = 0; i < n; i++) link(era(), rs[i]);
    return;
  }
  if (is_ext_value(p)) { free_val(p); return; }
  g_abort(AB_UNREACHABLE);
}

// APP-LAM beta: arg meets param, body meets ret
__device__ __noinline__ void beta(u64 app, u64 lam) {
  u32 ia = (u32)payload(app), il = (u32)payload(lam);
  u64 a0 = cell0(ia), a1 = cell1(ia), l0 = cell0(il), l1 = cell1(il);
  free_node(ia);
  free_node(il);
  link(a0, l0);
  link(a1, l1);
}

// OP-value: compute if the other operand has been produced, otherwise
// store this one and re-arm the op (flipped) against the missing operand
__device__ __noinline__ void op_rule(u64 op, u64 val) {
  u32 addr = op_addr(op);
  u16 code = op_code(op);
  u64 c0 = cell0(addr), c1 = cell1(addr);
  u64 other = resolve(c0);
  if (tag(other) == T_VAR) {
    cell_set(addr, 0, val);
    link(op_port(addr, code | OP_FLIP), other);
    return;
  }
  if (tag(other) == T_DUP) {
    // OP-SUP: the other operand is a superposition: one op per side
    u32 d = dup_addr(other);
    u32 label = dup_label(other);
    u64 d0 = cell0(d), d1 = cell1(d);
    free_node(d);
    free_node(addr);
    u64 v1 = wire(), v2 = wire();
    u32 dv = alloc_node(v1, v2);
    link(dup_port(dv, label), val);
    u64 r1 = wire(), r2 = wire();
    u32 dr = alloc_node(r1, r2);
    link(dup_port(dr, label), c1);
    u32 a1 = alloc_node(d0, r1);
    u32 a2 = alloc_node(d1, r2);
    link(op_port(a1, code), v1);
    link(op_port(a2, code), v2);
    return;
  }
  if (!is_value(other)) { g_abort(AB_UNREACHABLE); return; }
  u64 x = (code & OP_FLIP) ? other : val;
  u64 y = (code & OP_FLIP) ? val : other;
  u64 r;
  if (net_compute(code & 0xff, x, y, &r)) {
    free_node(addr);
    link(r, c1);
  } else {
    g_abort(AB_UNREACHABLE); // runtime: a builtin could not compute
  }
}

// SWI-NUM: fire the taken branch closure at the return port, erase the other
__device__ __noinline__ void swi_rule(u64 swi, u64 n) {
  u32 s = (u32)payload(swi);
  u64 ret = cell0(s), arms = cell1(s);
  free_node(s);
  u32 s2 = (u32)payload(arms);
  u64 t = cell0(s2), e = cell1(s2);
  free_node(s2);
  u64 taken = as_i(n) != 0 ? t : e;
  u64 dead = as_i(n) != 0 ? e : t;
  link(era(), dead);
  push_redex(taken, ret);
}

// MAT-constructor: select the arm whose ctor tag matches the scrutinee,
// prepend the fields to the arm closure's captures and fire it
__device__ __noinline__ void mat_rule(u64 mat, u64 val) {
  u32 m = mat_addr(mat);
  u16 mid = mat_id(mat);
  u64 ret = cell0(m), armlist = cell1(m);
  free_node(m);
  u64 fields[LISTCAP];
  int nf;
  u16 ct;
  if (tag(val) == T_CON) {
    ct = con_tag(val);
    nf = con_collect(val, fields);
  } else {
    // an unboxed constructor: its tag byte names the ctor, its field rides the payload
    u64 t = tag(val);
    if (t < TU) { g_abort(AB_UNREACHABLE); return; }
    ct = (u16)unbox_cid(t - TU);
    fields[0] = num(as_i(val));
    nf = 1;
  }
  usize pi;
  if (prog_mat_proj(mid, &pi)) {
    if (ct != CTAG_TUPLE || (int)pi >= nf) { g_abort(AB_UNREACHABLE); return; }
    for (int j = 0; j < nf; j++) {
      if ((usize)j == pi) link(fields[j], ret); else link(era(), fields[j]);
    }
    return;
  }
  int ntags;
  const u16 *tags = prog_mat_arms(mid, &ntags);
  u64 refs[LISTCAP];
  int nr = list_collect(armlist, refs);
  int j = -1;
  for (int i = 0; i < ntags; i++) if (tags[i] == ct) { j = i; break; }
  if (j < 0 || j >= nr) { g_abort(AB_UNREACHABLE); return; }
  for (int i = 0; i < nr; i++) if (i != j) link(era(), refs[i]);
  u64 rj = refs[j];
  if (tag(rj) != T_REF) { g_abort(AB_UNREACHABLE); return; }
  u64 args[LISTCAP];
  int na = 0;
  for (int i = 0; i < nf; i++) args[na++] = fields[i];
  u64 caps[LISTCAP];
  int nc = list_collect(ref_head(rj), caps);
  for (int i = 0; i < nc && na < LISTCAP; i++) args[na++] = caps[i];
  u64 head = list_alloc(args, na);
  push_redex(ref_port(head, ref_entry(rj)), ret);
}

// The DUP-LAM copy of the closure in cell l: two lambdas, their bodies
// joined by a label dup on the old body wire, their parameters by a
// same-label dup acting as the superposition. The first copy keeps cell l.
__device__ __noinline__ void copy_lam(u32 l, u32 label, u64 *l1, u64 *l2) {
  u64 c0 = cell0(l), c1 = cell1(l);
  u64 wp1 = wire(), wp2 = wire(), wb1 = wire(), wb2 = wire();
  u32 db = alloc_node(wb1, wb2);
  link(dup_port(db, label), c1);
  u32 su = alloc_node(wp1, wp2);
  link(dup_port(su, label), c0);
  cell_set(l, 0, wp1);
  cell_set(l, 1, wb1);
  u32 n2 = alloc_node(wp2, wb2);
  *l1 = mkport(T_LAM, l);
  *l2 = mkport(T_LAM, n2);
}

// The DUP-closure copy of an arm/branch closure r: two Refs to the same
// entry over dup'd captures
__device__ __noinline__ void copy_closure(u64 r, u32 label, u64 *ra, u64 *rb) {
  u64 caps[LISTCAP];
  int n = list_collect(ref_head(r), caps);
  u64 ca[LISTCAP], cb[LISTCAP];
  for (int i = 0; i < n; i++) {
    u64 w1 = wire(), w2 = wire();
    u32 nd = alloc_node(w1, w2);
    link(dup_port(nd, label), caps[i]);
    ca[i] = w1;
    cb[i] = w2;
  }
  u16 entry = ref_entry(r);
  *ra = ref_port(list_alloc(ca, n), entry);
  *rb = ref_port(list_alloc(cb, n), entry);
}

// DUP-value: copy
__device__ __noinline__ void dup_rule(u64 dup, u64 val) {
  u32 d = dup_addr(dup);
  u32 label = dup_label(dup);
  u64 o1 = cell0(d), o2 = cell1(d);
  free_node(d);
  u64 t = tag(val);
  if (t == T_NUM) {
    link(val, o1);
    link(val, o2);
  } else if (t == T_FLO) {
    u32 a = (u32)payload(val);
    u32 copy = alloc2(cell0(a), cell1(a)); // a new value: one reference
    link(val, o1);
    link(mkport(T_FLO, copy), o2);
  } else if (t == T_CON) {
    u16 ctag = con_tag(val);
    u64 fs[LISTCAP], fa[LISTCAP], fb[LISTCAP];
    int n = con_collect(val, fs);
    for (int i = 0; i < n; i++) {
      u64 w1 = wire(), w2 = wire();
      u32 df = alloc_node(w1, w2);
      link(dup_port(df, label), fs[i]);
      fa[i] = w1;
      fb[i] = w2;
    }
    link(con_alloc(ctag, fa, n), o1);
    link(con_alloc(ctag, fb, n), o2);
  } else if (t == T_LAM) {
    u64 l1, l2;
    copy_lam((u32)payload(val), label, &l1, &l2);
    link(l1, o1);
    link(l2, o2);
  } else {
    u64 copy = dup_val(val);
    link(val, o1);
    link(copy, o2);
  }
}

// a same-label dup whose outputs are fresh wires (an arm closure is copied
// right away)
__device__ __noinline__ void split(u64 p, u32 label, u64 *a, u64 *b) {
  if (tag(p) == T_REF) { copy_closure(p, label, a, b); return; }
  u64 w1 = wire(), w2 = wire();
  u32 nd = alloc_node(w1, w2);
  link(dup_port(nd, label), p);
  *a = w1;
  *b = w2;
}

// DUP-{OP, APP, SWI, MAT}: commute, the consumer passes through the dup
__device__ __noinline__ void dup_commute(u64 dup, u64 agent) {
  u32 d = dup_addr(dup);
  u32 label = dup_label(dup);
  u64 o1 = cell0(d), o2 = cell1(d);
  free_node(d);
  u64 t = tag(agent);
  if (t == T_OP) {
    u32 a = op_addr(agent);
    u16 code = op_code(agent);
    u64 c0 = cell0(a), c1 = cell1(a);
    free_node(a);
    u64 x1, x2, r1, r2;
    split(c0, label, &x1, &x2);
    split(c1, label, &r1, &r2);
    u32 a1 = alloc_node(x1, r1), a2 = alloc_node(x2, r2);
    link(op_port(a1, code), o1);
    link(op_port(a2, code), o2);
  } else if (t == T_APP) {
    u32 a = (u32)payload(agent);
    u64 c0 = cell0(a), c1 = cell1(a);
    free_node(a);
    u64 x1, x2, r1, r2;
    split(c0, label, &x1, &x2);
    split(c1, label, &r1, &r2);
    u32 a1 = alloc_node(x1, r1), a2 = alloc_node(x2, r2);
    link(mkport(T_APP, a1), o1);
    link(mkport(T_APP, a2), o2);
  } else if (t == T_SWI) {
    u32 s = (u32)payload(agent);
    u64 c0 = cell0(s), c1 = cell1(s);
    free_node(s);
    u32 s2 = (u32)payload(c1);
    u64 d0 = cell0(s2), d1 = cell1(s2);
    free_node(s2);
    u64 r1, r2, t1, t2, e1, e2;
    split(c0, label, &r1, &r2);
    split(d0, label, &t1, &t2);
    split(d1, label, &e1, &e2);
    u32 arms1 = alloc_node(t1, e1), arms2 = alloc_node(t2, e2);
    u32 n1 = alloc_node(r1, mkport(T_EXT, arms1)), n2 = alloc_node(r2, mkport(T_EXT, arms2));
    link(mkport(T_SWI, n1), o1);
    link(mkport(T_SWI, n2), o2);
  } else if (t == T_MAT) {
    u32 m = mat_addr(agent);
    u16 id = mat_id(agent);
    u64 c0 = cell0(m), c1 = cell1(m);
    free_node(m);
    u64 r1, r2;
    split(c0, label, &r1, &r2);
    u64 arms[LISTCAP], l1[LISTCAP], l2[LISTCAP];
    int n = list_collect(c1, arms);
    for (int i = 0; i < n; i++) split(arms[i], label, &l1[i], &l2[i]);
    u64 h1 = list_alloc(l1, n), h2 = list_alloc(l2, n);
    u32 n1 = alloc_node(r1, h1), n2 = alloc_node(r2, h2);
    link(mat_port(n1, id), o1);
    link(mat_port(n2, id), o2);
  } else {
    g_abort(AB_UNREACHABLE);
  }
}

// DUP-DUP: same label annihilate, different labels commute
__device__ __noinline__ void dup_dup(u64 a, u64 b) {
  u32 ia = dup_addr(a), ib = dup_addr(b);
  u32 la = dup_label(a), lb = dup_label(b);
  u64 a0 = cell0(ia), a1 = cell1(ia), b0 = cell0(ib), b1 = cell1(ib);
  free_node(ia);
  free_node(ib);
  if (la == lb) {
    link(a0, b0);
    link(a1, b1);
    return;
  }
  u64 w0 = wire(), w1 = wire(), w2 = wire(), w3 = wire();
  u32 bb1 = alloc_node(w0, w1), bb2 = alloc_node(w2, w3);
  u32 aa1 = alloc_node(w0, w2), aa2 = alloc_node(w1, w3);
  link(dup_port(bb1, lb), a0);
  link(dup_port(bb2, lb), a1);
  link(dup_port(aa1, la), b0);
  link(dup_port(aa2, la), b1);
}

// Process one redex: 0 for pure wiring, 1 for a rule firing
__device__ __noinline__ int process(u64 a, u64 b) {
  u64 ta = tag(a), tb = tag(b);
  if (ta == T_REF || tb == T_REF) {
    u64 r = ta == T_REF ? a : b, other = ta == T_REF ? b : a;
    if (tag(other) == T_REF) { g_abort(AB_UNREACHABLE); return 1; }
    if (tag(other) == T_ERA) {
      u64 ps[LISTCAP];
      int n = list_collect(ref_head(r), ps);
      for (int i = 0; i < n; i++) link(era(), ps[i]);
      return 1;
    }
    if (tag(other) == T_DUP && is_closure(r)) {
      u32 d = dup_addr(other);
      u32 label = dup_label(other);
      u64 d0 = cell0(d), d1 = cell1(d);
      free_node(d);
      u64 ra, rb;
      copy_closure(r, label, &ra, &rb);
      link(ra, d0);
      link(rb, d1);
      return 1;
    }
    unfold(r, other);
    return 1;
  }
  if (ta == T_VAR || tb == T_VAR) { link(a, b); return 0; }
  if (ta == T_KONT || tb == T_KONT) {
    u64 k = ta == T_KONT ? a : b, v = ta == T_KONT ? b : a;
    if (!is_value(v)) { g_abort(AB_UNREACHABLE); return 1; }
    // compiled code reads the value whole: delivered once settled
    u32 j;
    if (settled(v, WHOLE_RULE, (u32)v, (u32)(v >> 32), payload(k), &j)) deliver(payload(k), v);
    return 1;
  }
  if (ta == T_ERA || tb == T_ERA) {
    era_value(ta == T_ERA ? b : a);
    return 1;
  }
  if (ta == T_APP && tb == T_LAM) beta(a, b);
  else if (ta == T_LAM && tb == T_APP) beta(b, a);
  else if (ta == T_OP && is_value(b)) op_rule(a, b);
  else if (tb == T_OP && is_value(a)) op_rule(b, a);
  else if (ta == T_SWI && tb == T_NUM) swi_rule(a, b);
  else if (ta == T_NUM && tb == T_SWI) swi_rule(b, a);
  else if (ta == T_MAT && is_value(b)) mat_rule(a, b);
  else if (tb == T_MAT && is_value(a)) mat_rule(b, a);
  else if (ta == T_DUP && tb == T_DUP) dup_dup(a, b);
  else if (ta == T_DUP && is_value(b)) dup_rule(a, b);
  else if (tb == T_DUP && is_value(a)) dup_rule(b, a);
  else if (ta == T_DUP && (tb == T_OP || tb == T_APP || tb == T_SWI || tb == T_MAT)) dup_commute(a, b);
  else if (tb == T_DUP && (ta == T_OP || ta == T_APP || ta == T_SWI || ta == T_MAT)) dup_commute(b, a);
  else g_abort(AB_UNREACHABLE);
  return 1;
}

// Fire the rule table on the lane's queued redexes until none is left or
// the budget is spent; the rest is spawned to the net rule (a later wave)
__device__ __noinline__ void reduce_net() {
  int budget = G.net_fuel;
  int done = 0;
  u64 a, b;
  while (done < budget) {
    if (!pop_redex(&a, &b)) return;
    done += process(a, b);
  }
  while (pop_redex(&a, &b)) spawn3(NET_RULE, a, b, 0);
}
__device__ inline void net_push(u64 a, u64 b) { push_redex(a, b); }

// NET_RULE: a generic redex spilled to the engine
__device__ __noinline__ void net_fire(u64 a, u64 b) {
  net_push(a, b);
  reduce_net();
}
// FILL_RULE: a compiled call's result links into the wire the net waits on
__device__ __noinline__ void fill_fire(u64 a, u64 aux) {
  u32 ri = (u32)aux;
  u64 target = (u64)rec_d(ri) | ((u64)rec_s(ri) << 32);
  link(a, target);
  reduce_net();
}

// ---- settling a net value compiled code will read (rules::settle) ----

// a constructor with fields joins the settle worklist; false (run aborted)
// when the worklist is full
__device__ inline bool settle_push(u64 *work, int *nw, u64 f) {
  if (tag(f) != T_CON || con_ar(f) == 0) return true;
  if (*nw == LISTCAP) { g_abort(AB_UNSUPPORTED); return false; }
  work[(*nw)++] = f;
  return true;
}
// Resolve every constructor field of `v` in place, transitively. Each field
// still waiting on an unfilled wire gets a FIELD record fed by that wire and
// reporting to *j; *j (rule, d, s, parent) is allocated on the first one,
// with one guard count so no arrival can complete it during the scan (see
// settle_release). The head is visited before the tail, so the worklist
// grows with left nesting only, not with a list's length. The number of
// fields awaited, or -1 (run aborted) past LISTCAP of left nesting.
__device__ __noinline__ int settle_await(u64 v, u16 rule, u32 d, u32 s, u64 parent, u32 *j) {
  u64 work[LISTCAP];
  int nw = 0, np = 0;
  if (!settle_push(work, &nw, v)) return -1;
  while (nw) {
    u64 p = work[--nw];
    u32 c = con_addr(p);
    int ar = con_ar(p);
    u64 *slot = &G.nodes[2 * (u64)nclamp(c)];
    // pushed first, visited last: the chain rest (arity > 2), then slot 1
    if (ar > 2 && !settle_push(work, &nw, slot[1])) return -1;
    for (int k = (ar == 2 ? 1 : 0); k >= 0; k--) {
      u64 f = slot[k];
      if (tag(f) == T_VAR) slot[k] = f = resolve(f);
      if (tag(f) == T_VAR) {
        if (*j == NOREC) *j = alloc_rec(rule, 1, d, s, parent);
        atomicAdd(&G.recs[rclamp(*j)].pend, 1);
        u32 fr = alloc_rec(FIELD_RULE, 1, c, (u32)k, rec_addr(*j));
        link(kont_port(rec_addr(fr)), f);
        np++;
      } else if (!settle_push(work, &nw, f)) {
        return -1;
      }
    }
  }
  return np;
}
// Drop the scan's guard on record j: true when every awaited field already
// arrived (the value is whole; j is freed unfired), false when the last
// arrival will fire j.
__device__ bool settle_release(u32 j) {
  if (atomicSub(&G.recs[rclamp(j)].pend, 1) != 1) return false;
  rec_free(j);
  return true;
}
// `v` settled: true. Otherwise a `rule` record (d, s, parent) waits for its
// pending fields and false is returned, the record in *j.
__device__ bool settled(u64 v, u16 rule, u32 d, u32 s, u64 parent, u32 *j) {
  *j = NOREC;
  if (settle_await(v, rule, d, s, parent, j) < 0) return true;
  return *j == NOREC || settle_release(*j);
}
// FIELD_RULE: a pending field's value arrived (whole: it came through a
// Kont, which settles first): written into its slot
__device__ __noinline__ void field_fire(u64 a, u64 aux) {
  u32 ri = (u32)aux;
  G.nodes[2 * (u64)nclamp(rec_d(ri)) + rec_s(ri)] = a;
  deliver(rec_parent(ri), 0);
}
// WHOLE_RULE: every field arrived: the value goes to its destination
__device__ __noinline__ void whole_fire(u64 aux) {
  u32 ri = (u32)aux;
  deliver(rec_parent(ri), (u64)rec_d(ri) | ((u64)rec_s(ri) << 32));
}
// RELINK_RULE: a call's arguments are whole: the call meets its output again
__device__ __noinline__ void relink_fire(u64 aux) {
  u32 ri = (u32)aux;
  // a redex, not a link: a call against a wire must unfold, not park
  push_redex((u64)rec_d(ri) | ((u64)rec_s(ri) << 32), rec_parent(ri));
  reduce_net();
}

// ---- the bridge compiled code uses ----

__device__ __noinline__ u64 dup_closure(u64 p) {
  u32 label = fresh_label();
  u64 l1, l2;
  copy_lam((u32)(p & M56), label, &l1, &l2);
  reduce_net();
  return l2;
}
__device__ __noinline__ void drop_closure(u64 p) {
  link(era(), p);
  reduce_net();
}
__device__ __noinline__ u64 build_closure(u16 id, const u64 *caps, int n) {
  u64 w = wire();
  prog_inst(id, caps, n, w);
  reduce_net();
  return resolve(w);
}
// Apply a closure value from compiled code: a result within budget is
// returned, otherwise the dive suspends on a forwarding record
__device__ __noinline__ R apply(u64 f, u64 a) {
  if (tag(f) != T_LAM) { g_abort(AB_UNREACHABLE); return R{0, true}; }
  u64 w = wire();
  u32 c = alloc_node(a, w);
  link(mkport(T_APP, c), f);
  reduce_net();
  u64 v = resolve(w);
  if (tag(v) != T_VAR) {
    // compiled code reads the result whole
    u32 j;
    if (settled(v, WHOLE_RULE, (u32)v, (u32)(v >> 32), NONE, &j)) return R{v, true};
    return R{(u64)j, false};
  }
  u32 r = alloc_rec(FWD_RULE, 1, 0, 0, NONE);
  link(kont_port(rec_addr(r)), v);
  return R{(u64)r, false};
}
__device__ __noinline__ void apply_spawn(u64 f, u64 a, u64 parent) {
  if (tag(f) != T_LAM) { g_abort(AB_UNREACHABLE); return; }
  u32 c = alloc_node(a, kont_port(parent));
  link(mkport(T_APP, c), f);
  reduce_net();
}

// ---- the kernels: boot, and the driver ----

// Fire one task: the engine's ERA rule erases a value; a program rule
// fires through the table, and a fired record is dead.
__device__ inline void fire(u32 rule, u64 e0, u64 e1, u64 e2) {
  if (rule == ERA_RULE) {
    if (e1) arr_erase(e0, e1 - 1); else free_val(e0);
    return;
  }
  prog_fire(rule, e0, e1, e2);
  if (prog_rec_rule(rule)) rec_free((u32)e2);
}

// Run the joins this lane completed (parallel world: outside k_work the
// local stack only ever holds completed cross-lane joins).
__device__ void drain_local() {
  u32 L = lane();
  while (G.lsn[L]) {
    if (*(volatile u32 *)G.abortf != 0)
      return;
    u32 n = G.lsn[L];
    u64 *t = &G.lstk[((u64)L * LSCAP + n - 1) * 4];
    u32 rule = (u32)t[0] & ~PAR_TASK;
    u64 e0 = t[1], e1 = t[2], e2 = t[3];
    G.lsn[L] = n - 1;
    fire(rule, e0, e1, e2);
  }
}

extern "C" __global__ void k_boot(u64 a, u64 b, u64 c, int fuel) {
  stack_mark();
  s_mode[threadIdx.x] = 0;
  s_fuel = fuel;
  prog_fire(0, a, b, c);
  drain_local();
}

// Fire one global task in the current world; a fired record is dead, and
// the joins and ready records its firing completed run at once on this lane.
__device__ inline void fire_task(u32 rule, u32 idx) {
  const u64 *e = &G.ebuf[((u64)rule * G.bcap + (idx & (G.bcap - 1))) * 3];
  fire(rule, e[0], e[1], e[2]);
  drain_local();
}

// WORK phase: every lane drains its own tasks depth-first (the tasks it
// spawns and the joins it completes stay on it), dealt every nl-th pending
// task of the snapshot. After `max_steps` fires a lane spills its stack to
// the global rings and stops; a fire's dive forms yield every WORK_CAP
// units (its native loops run to their ends).
#define LOGCAP (1u << 16)
__device__ u32 g_off[NRULES_ALL + 1]; // forkable-task prefix of the snapshot
__device__ u32 g_woff[NRULES_ALL + 1]; // pending-task prefix of the snapshot (WORK deals these)
__device__ u32 g_snap[NRULES_ALL];    // blen at the snapshot
__device__ u32 g_wmax, g_wsum;         // a work phase: the most steps one lane took, all lanes' steps
__device__ u32 g_wcyc, g_wbusy, g_wsteps_of_max; // a work phase: the slowest lane's K cycles, lanes that fired, its steps
__device__ u32 g_whist[40];            // lanes per log2(K cycles) bucket, the last work phase (trace)
__device__ u32 g_wlog[LOGCAP * 6];     // per round: work steps (max lane, sum), K cycles, slowest lane K cycles, its steps, busy lanes; trace
__device__ int g_phase;                // 0 exit, 1 grow, 2 work
__device__ int g_grew = 1;
__device__ u64 g_prev = 0;             // total pushes at the previous snapshot
__device__ u32 g_log[LOGCAP * 3];      // per round: phase, pending, forkable (trace)
#define RLOG 64
__device__ u32 g_rlog[RLOG * NRULES_ALL]; // the first rounds' pending tasks per rule (trace)

// The sequential world runs a lane's subtrees with the dive budget: a
// fuel-out suspension costs ~100 rewrites' worth of allocation (bitonic
// depth 23: 0.84 s -> 1.03 s against an unbounded budget), but the budget
// is also the only bound on native recursion depth, and the device stack
// is 32 KiB per thread.
__device__ void work_phase(u32 max_steps) {
  s_mode[threadIdx.x] = 1;
  s_fuel = G.fuel;
  __syncthreads();
  u32 L = lane();
  const u32 nl = gridDim.x * blockDim.x;
  const u32 gid = blockIdx.x * blockDim.x + threadIdx.x;
  u32 s = 0;
  long long w0 = clock64();
  // this lane's column: every nl-th pending task of the snapshot (dealt,
  // not claimed: no race, every lane gets its share), each drained
  // depth-first with what it spawns and the joins it completes
  u32 ntask = g_woff[G.nrules];
  u32 next = gid;
  for (; s < max_steps; s++) {
    if (*(volatile u32 *)G.abortf != 0)
      return;
    u32 rule;
    u64 e0, e1, e2;
    u32 n = G.lsn[L];
    if (n) {
      u64 *t = &G.lstk[((u64)L * LSCAP + n - 1) * 4];
      rule = (u32)t[0];
      e0 = t[1];
      e1 = t[2];
      e2 = t[3];
      G.lsn[L] = n - 1;
      if (rule & PAR_TASK) {
        rule &= ~PAR_TASK;
        s_mode[threadIdx.x] = 0; // a cross-lane join: the parallel world
      }
    } else {
      if (next >= ntask)
        break; // this lane's column is drained
      u32 r = 0;
      while (next >= g_woff[r + 1]) r++;
      u32 idx = G.bdone[r] + (next - g_woff[r]);
      const u64 *e = &G.ebuf[((u64)r * G.bcap + (idx & (G.bcap - 1))) * 3];
      rule = r;
      e0 = e[0];
      e1 = e[1];
      e2 = e[2];
      next += nl;
    }
    fire(rule, e0, e1, e2);
    s_mode[threadIdx.x] = 1;
  }
  // step cap: hand the unfinished local tasks, and the dealt tasks this
  // lane never started, back to the global rings (the leader then moves
  // bdone past the snapshot)
  u32 n = G.lsn[L];
  for (u32 i = 0; i < n; i++) {
    u64 *t = &G.lstk[((u64)L * LSCAP + i) * 4];
    spawn_global((u32)t[0] & ~PAR_TASK, t[1], t[2], t[3]);
  }
  G.lsn[L] = 0;
  for (; next < ntask; next += nl) {
    u32 r = 0;
    while (next >= g_woff[r + 1]) r++;
    u32 idx = G.bdone[r] + (next - g_woff[r]);
    const u64 *e = &G.ebuf[((u64)r * G.bcap + (idx & (G.bcap - 1))) * 3];
    spawn_global(r, e[0], e[1], e[2]);
  }
  if (s) {
    atomicMax(&g_wmax, s);
    atomicAdd(&g_wsum, s);
    atomicAdd(&g_wbusy, 1);
    u32 kc = (u32)((clock64() - w0) >> 10);
    if (atomicMax(&g_wcyc, kc) < kc) g_wsteps_of_max = s;
    atomicAdd(&g_whist[kc ? 32 - __clz(kc) : 0], 1u);
  }
}

// ---- the driver on the device ----
//
// One cooperative launch runs the whole program in a grow/work rhythm adopted from
// reference's runtime design (design.md s14; to be replaced). A round:
// while the frontier (pending global tasks) is narrower than the lanes and
// some pending task can fork, GROW sweeps: every forkable task below a
// snapshot fires in the parallel world, one grid barrier per sweep, until
// the frontier is wide or a sweep grew nothing; then one WORK phase: every
// lane drains its column in the sequential world. It stops when nothing is
// pending (the root delivered) or the run aborted. The host launches once.

extern "C" __global__ void k_run(u32 grow_width, u32 work_steps, int grow_fuel, u64 max_rounds) {
  cg::grid_group grid = cg::this_grid();
  stack_mark();
  const u32 nl = gridDim.x * blockDim.x;
  const u32 gid = blockIdx.x * blockDim.x + threadIdx.x;
  for (;;) {
    if (gid == 0) {
      u32 total = 0, forkable = 0, off = 0;
      for (u32 r = 0; r < G.nrules; r++) {
        u32 len = *(volatile u32 *)&G.blen[r];
        u32 pend = len - G.bdone[r];
        g_snap[r] = len;
        g_off[r] = off;
        g_woff[r] = total;
        if (g_rounds[0] < RLOG) g_rlog[g_rounds[0] * NRULES_ALL + r] = pend;
        if (rule_forks(r)) {
          off += pend;
          forkable += pend;
        }
        total += pend;
      }
      g_off[G.nrules] = off;
      g_woff[G.nrules] = total;
      // reference's rule (adopted): a grow sweep grew when it pushed anything (some task
      // forked); growth stops when a sweep forks nothing or the frontier
      // is as wide as the lanes
      u64 pushed = 0;
      for (u32 r = 0; r < G.nrules; r++) pushed += g_snap[r];
      u64 f = pushed - g_prev; // the frontier: tasks pushed by the last phase
      if (g_phase == 1) g_grew = f > 0;
      g_prev = pushed;
      if (total > g_rounds[3]) g_rounds[3] = total;
      if (g_rounds[0] >= max_rounds) g_abort(AB_ROUNDS);
      if (*(volatile u32 *)G.abortf != 0 || total == 0)
        g_phase = 0;
      else if (forkable > 0 && f < grow_width && g_grew)
        g_phase = 1;
      else
        g_phase = 2;
      g_wmax = g_wsum = g_wcyc = g_wbusy = g_wsteps_of_max = 0;
      if (g_phase == 2) for (int k = 0; k < 40; k++) g_whist[k] = 0;
      if (g_rounds[0] < LOGCAP) {
        g_log[g_rounds[0] * 3] = g_phase;
        g_log[g_rounds[0] * 3 + 1] = total;
        g_log[g_rounds[0] * 3 + 2] = (u32)f;
      }
      g_rounds[0]++;
    }
    grid.sync();
    int ph = *(volatile int *)&g_phase;
    if (ph == 0) return;
    long long c0 = clock64();
    if (ph == 1) {
      s_mode[threadIdx.x] = 0;
      s_fuel = grow_fuel; // >= 1 (the runner clamps it): a task's own entry must run, or it would re-spawn itself forever
      __syncthreads();
      u32 n = g_off[G.nrules];
      for (u32 i = gid; i < n; i += nl) {
        if (*(volatile u32 *)G.abortf != 0) break;
        u32 r = 0;
        while (i >= g_off[r + 1]) r++;
        fire_task(r, G.bdone[r] + (i - g_off[r]));
      }
      grid.sync();
      if (gid == 0) {
        for (u32 r = 0; r < G.nrules; r++)
          if (rule_forks(r)) G.bdone[r] = g_snap[r];
        g_rounds[1]++;
        g_rounds[4] += clock64() - c0;
        u64 i = g_rounds[0] - 1;
        if (i < LOGCAP) g_wlog[i * 6 + 2] = (u32)((clock64() - c0) >> 10);
      }
    } else {
      work_phase(work_steps);
      grid.sync();
      if (gid == 0) {
        for (u32 r = 0; r < G.nrules; r++) G.bdone[r] = g_snap[r];
        g_grew = 1;
        g_rounds[2]++;
        g_rounds[5] += clock64() - c0;
        u64 i = g_rounds[0] - 1;
        if (i < LOGCAP) {
          g_wlog[i * 6] = g_wmax;
          g_wlog[i * 6 + 1] = g_wsum;
          g_wlog[i * 6 + 2] = (u32)((clock64() - c0) >> 10);
          g_wlog[i * 6 + 3] = g_wcyc;
          g_wlog[i * 6 + 4] = g_wsteps_of_max;
          g_wlog[i * 6 + 5] = g_wbusy;
        }
      }
    }
    grid.sync();
  }
}

