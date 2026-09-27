// mithril-gpu: fixed device-side wave engine (program-independent).
//
// A generated program.cu does:
//     #define PROG_NRULES <n>
//     #include "engine.cu"
//     ... __device__ void prog_fire(...) and __device__ u64 prog_dive(...)
// and this file supplies the alloc / deliver / bucket machinery plus the
// kernels the host wave loop launches. Protocol mirrors the CPU engine
// (mithril-rt): the boot redex fires as rule 0, record 0 is the reserved
// ROOT result sink (parent address 0), a record whose pend counter reaches
// zero is queued (never fired inline) into its rule's bucket with the
// record index in the entry's third word.
//
// Memory safety after an abort: alloc returns cell 0 / the last record, and
// every cell/record read is clamped into the arena, so a poisoned run can
// produce garbage values but never an illegal access; the host reads the
// abort flag each wave and turns it into a clean error.

typedef unsigned long long u64;
typedef unsigned int u32;
typedef long long i64;

#define SUSP 0xffffffffffffffffull
#define DIVE_FUEL 64
#define MAXLANES (1 << 16)
#define FREECAP 64

// abort codes (host maps >= AB_ARENA to "arena exhausted"); arena outranks
// unreachable-match via atomicMax so the root cause wins.
#define AB_UNREACHABLE 1u
#define AB_ARENA 2u

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
  u32 ncap, rcap, bcap, ovfcap, chunksz, nrules;
};

extern "C" {
__device__ Dev G;
__device__ u32 g_nrules = PROG_NRULES;
}

__device__ void prog_fire(u32 rule, u64 e0, u64 e1, u64 e2);

__device__ inline void g_abort(u32 code) { atomicMax(G.abortf, code); }

__device__ inline u32 lane() {
  return (blockIdx.x * blockDim.x + threadIdx.x) & (MAXLANES - 1);
}

// ---- ports: tag:8 | payload:56; CON payload: addr:40 | ctor:12 | arity:4 ----

#define TAG_NUM 2u
#define TAG_FLO 3u
#define TAG_CON 4u
#define TUPLE_CTOR 0xfffu

__device__ inline u32 ptag(u64 p) { return (u32)(p >> 56); }
__device__ inline u64 mk_num(i64 v) {
  return ((u64)TAG_NUM << 56) | ((u64)v & 0x00ffffffffffffffull);
}
__device__ inline i64 as_i(u64 p) { return ((i64)(p << 8)) >> 8; }
__device__ inline u64 mk_con(u32 addr, u32 ctor, u32 arity) {
  u32 ar = arity > 15u ? 15u : arity;
  return ((u64)TAG_CON << 56) | ((u64)addr << 16) | ((u64)(ctor & 0xfffu) << 4) | ar;
}
__device__ inline u32 con_addr(u64 p) { return (u32)(p >> 16); }
__device__ inline u32 con_tag(u64 p) { return ((u32)p >> 4) & 0xfffu; }

// ---- clamped cell access (safe even on garbage after an abort) ----

__device__ inline u32 nclamp(u32 i) { return i < G.ncap ? i : G.ncap - 1; }
__device__ inline u64 cell0(u32 i) { return G.nodes[2 * (u64)nclamp(i)]; }
__device__ inline u64 cell1(u32 i) { return G.nodes[2 * (u64)nclamp(i) + 1]; }
__device__ inline void setcell(u32 i, u64 a, u64 b) {
  u32 j = nclamp(i);
  G.nodes[2 * (u64)j] = a;
  G.nodes[2 * (u64)j + 1] = b;
}

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

// ---- records / buckets / delivery ----

__device__ u32 alloc_rec(u32 rule, int pend, u32 d, u32 s, u64 parent) {
  u32 i = atomicAdd(G.rbump, 1);
  if (i >= G.rcap) {
    g_abort(AB_ARENA);
    return G.rcap - 1;
  }
  Rec &r = G.recs[i];
  r.pend = pend;
  r.rule = (unsigned short)rule;
  r.s = (unsigned short)s;
  r.d = d;
  r.parent = parent;
  return i;
}

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

// ---- shared numeric semantics (i56 wrapping ints, boxed f64) ----

__device__ inline i64 wrap56(i64 v) { return ((i64)((u64)v << 8)) >> 8; }

__device__ u64 mk_flo(double x) {
  return ((u64)TAG_FLO << 56) | alloc_node(__double_as_longlong(x), 0);
}
__device__ inline double as_f(u64 p) {
  return __longlong_as_double(cell0((u32)(p & 0x00ffffffffffffffull)));
}

// op: 0 Add 1 Sub 2 Mul 3 Div 4 FloorDiv 5 Mod 6 Shl 7 Shr 8 And 9 Or 10 Xor
__device__ u64 d_op2(u32 op, u64 a, u64 b) {
  if (ptag(a) == TAG_FLO || ptag(b) == TAG_FLO) {
    if (ptag(a) != TAG_FLO || ptag(b) != TAG_FLO) {
      g_abort(AB_UNREACHABLE);
      return mk_num(0);
    }
    double x = as_f(a), y = as_f(b), r = 0.0;
    switch (op) {
    case 0: r = x + y; break;
    case 1: r = x - y; break;
    case 2: r = x * y; break;
    case 3: r = x / y; break;
    default: g_abort(AB_UNREACHABLE); break;
    }
    return mk_flo(r);
  }
  i64 x = as_i(a), y = as_i(b), r = 0;
  switch (op) {
  case 0: r = (i64)((u64)x + (u64)y); break;
  case 1: r = (i64)((u64)x - (u64)y); break;
  case 2: r = (i64)((u64)x * (u64)y); break;
  case 3: r = y ? x / y : 0; break;
  case 4: {
    if (!y) break;
    i64 q = x / y, m = x % y;
    r = (m && ((m < 0) != (y < 0))) ? q - 1 : q;
    break;
  }
  case 5: {
    if (!y) break;
    i64 m = x % y;
    r = (m && ((m < 0) != (y < 0))) ? m + y : m;
    break;
  }
  case 6: r = (i64)((u64)x << ((u32)y & 63u)); break;
  case 7: r = x >> ((u32)y & 63u); break;
  case 8: r = x & y; break;
  case 9: r = x | y; break;
  case 10: r = x ^ y; break;
  }
  return mk_num(wrap56(r));
}

// op: 0 Lt 1 Le 2 Gt 3 Ge 4 Eq 5 Ne
__device__ u64 d_cmp(u32 op, u64 a, u64 b) {
  bool r = false;
  if (ptag(a) == TAG_FLO && ptag(b) == TAG_FLO) {
    double x = as_f(a), y = as_f(b);
    switch (op) {
    case 0: r = x < y; break;
    case 1: r = x <= y; break;
    case 2: r = x > y; break;
    case 3: r = x >= y; break;
    case 4: r = x == y; break;
    case 5: r = x != y; break;
    }
  } else {
    i64 x = as_i(a), y = as_i(b);
    switch (op) {
    case 0: r = x < y; break;
    case 1: r = x <= y; break;
    case 2: r = x > y; break;
    case 3: r = x >= y; break;
    case 4: r = x == y; break;
    case 5: r = x != y; break;
    }
  }
  return mk_num(r ? 1 : 0);
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
