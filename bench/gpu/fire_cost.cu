// Standalone cost model of one device "fire" (the unit of work the wave
// engine schedules), built on the real runtime (cuda/engine.cu) with a
// stub program. It reproduces the per-fire mix measured on tree-bitonic's
// sequential tail (409,599 pump fires at ~49k cycles: ~24 cell allocs,
// ~24 frees, ~31 cell reads, one record, one spawn, one delivery) and
// prices each component alone, on one thread (the pump) and on a full
// grid (a wide wave). Numbers are cycles per fire (clock64) and wall time.
//
// Build/run (in the nvcc container, from the repo root):
//   nvcc -O3 -arch=sm_89 -I crates/mithril-gpu/cuda bench/gpu/fire_cost.cu -o /tmp/fire_cost && /tmp/fire_cost
#include <cstdio>
#include <cstdlib>
#include <cuda_runtime.h>

#define PROG_NRULES 8
#define NFNS 1
#define NET_RULE 6
#define FILL_RULE 7
#define FWD_RULE 5
#include "engine.cu"

__device__ const bool REC_RULE[PROG_NRULES] = {false, true, true, true, true, true, false, true};
__device__ bool prog_rec_rule(u32 rule) { return REC_RULE[rule]; }
__device__ bool lin(u16 k) { return k == 1; } // ctor 1 linear, ctor 2 refcounted
__device__ u32 unbox_cid(u64 slot) { return 0; }
__device__ void prog_inst(u16, const u64 *, int, u64) {}
__device__ bool prog_mat_proj(u16, usize *) { return false; }
__device__ const u16 *prog_mat_arms(u16, int *n) { *n = 0; return 0; }
__device__ R prog_dive(u32, const u64 *, i64 *) { return R{0, true}; }
__device__ void prog_fire(u32, u64, u64, u64) {}

// ---- allocator variants (copies of engine.cu's, one change each) ----
// V0: the engine's alloc_node/free_node as they are.
// V1: the bump path does not probe the overflow ring (two atomics) unless
//     the ring is non-empty (a plain load first).
// V2: V1 + the lane's counters (free-list length, chunk cursor) in shared
//     memory instead of global.
// V3: V1 + the lane's whole state (counters + free list) in registers for
//     the duration of a fire (what a per-fire context would give).
__shared__ u32 s_nfreen[256];
__shared__ u32 s_chunk[512];

__device__ __noinline__ u32 alloc_v1(u64 a, u64 b) {
  u32 L = lane();
  if (G.nfreen[L]) {
    u32 i = G.nfree[L * FREECAP + --G.nfreen[L]];
    setcell(i, a, b);
    return i;
  }
  if (*(volatile int *)G.ovftop > 0) {
    int t = atomicSub(G.ovftop, 1) - 1;
    if (t >= 0 && (u32)t < G.ovfcap) {
      u32 v = atomicExch(&G.ovf[t], 0u);
      if (v) { setcell(v - 1, a, b); return v - 1; }
    }
    atomicAdd(G.ovftop, 1);
  }
  u32 *ck = &G.nchunk[2 * L];
  if (ck[0] >= ck[1]) {
    u32 base = atomicAdd(G.nbump, G.chunksz);
    ck[0] = base;
    ck[1] = base + G.chunksz;
  }
  u32 i = ck[0]++;
  setcell(i, a, b);
  return i;
}
__device__ __noinline__ u32 alloc_v2(u64 a, u64 b) {
  u32 L = lane();
  u32 t = threadIdx.x;
  if (s_nfreen[t]) {
    u32 i = G.nfree[L * FREECAP + --s_nfreen[t]];
    setcell(i, a, b);
    return i;
  }
  if (s_chunk[2 * t] >= s_chunk[2 * t + 1]) {
    u32 base = atomicAdd(G.nbump, G.chunksz);
    s_chunk[2 * t] = base;
    s_chunk[2 * t + 1] = base + G.chunksz;
  }
  u32 i = s_chunk[2 * t]++;
  setcell(i, a, b);
  return i;
}
__device__ __noinline__ void free_v2(u32 i) {
  u32 L = lane();
  u32 t = threadIdx.x;
  if (s_nfreen[t] < FREECAP) G.nfree[L * FREECAP + s_nfreen[t]++] = i;
}
struct LaneState { u32 nfreen; u32 cur, end; u32 fl[16]; };
__device__ inline u32 alloc_v3(LaneState *ls, u64 a, u64 b) {
  if (ls->nfreen) {
    u32 i = ls->fl[--ls->nfreen];
    setcell(i, a, b);
    return i;
  }
  if (ls->cur >= ls->end) {
    u32 base = atomicAdd(G.nbump, G.chunksz);
    ls->cur = base;
    ls->end = base + G.chunksz;
  }
  u32 i = ls->cur++;
  setcell(i, a, b);
  return i;
}
__device__ inline void free_v3(LaneState *ls, u32 i) {
  if (ls->nfreen < 16) ls->fl[ls->nfreen++] = i;
}

// what a fire does: bits select components so each can be priced alone
#define C_ALLOC 1
#define C_FREE 2
#define C_READ 4
#define C_REC 8
#define C_SPAWN 16
#define C_DELIVER 32
#define C_ALL 63

__device__ unsigned long long g_sink;

// one synthetic fire with the bitonic mix; `mask` selects the components,
// `variant` the allocator
__device__ __noinline__ void fire_sim(u32 mask, u32 seed, u32 variant) {
  u64 cells[24];
  u64 acc = 0;
  LaneState ls;
  ls.nfreen = 0; ls.cur = 0; ls.end = 0;
  if (mask & C_ALLOC) {
    for (int i = 0; i < 24; i++) {
      u32 c;
      switch (variant) {
      case 1: c = alloc_v1(num(i), num(seed)); break;
      case 2: c = alloc_v2(num(i), num(seed)); break;
      case 3: c = alloc_v3(&ls, num(i), num(seed)); break;
      default: c = alloc_node(num(i), num(seed)); break;
      }
      cells[i] = con(c, 2, 2);
    }
  } else {
    for (int i = 0; i < 24; i++) cells[i] = con(1 + ((seed * 7 + i) & 0xffff), 2, 2);
  }
  if (mask & C_READ) {
    for (int i = 0; i < 31; i++) acc += cell0(con_addr(cells[i % 24])) + cell1(con_addr(cells[(i * 5) % 24]));
  }
  if (mask & C_FREE) {
    for (int i = 0; i < 24; i++) {
      switch (variant) {
      case 2: free_v2(con_addr(cells[i])); break;
      case 3: free_v3(&ls, con_addr(cells[i])); break;
      default: free_node(con_addr(cells[i])); break;
      }
    }
  }
  u32 r = 1;
  if (mask & C_REC) r = alloc_rec(3, 1, 0, 0, NONE);
  if (mask & C_SPAWN) spawn3(2, num(1), num(2), rec_addr(r));
  if (mask & C_DELIVER) deliver(rec_addr(r), num(acc));
  if (mask & C_REC) rec_free(r);
  g_sink += acc;
}

extern "C" __global__ void k_seq(u32 steps, u32 mask, u32 variant, unsigned long long *cyc) {
  s_nfreen[threadIdx.x] = 0; s_chunk[2 * threadIdx.x] = 0; s_chunk[2 * threadIdx.x + 1] = 0;
  long long c0 = clock64();
  for (u32 s = 0; s < steps; s++) fire_sim(mask, s, variant);
  *cyc = clock64() - c0;
}

extern "C" __global__ void k_par(u32 count, u32 mask, u32 variant, unsigned long long *cyc) {
  s_nfreen[threadIdx.x] = 0; s_chunk[2 * threadIdx.x] = 0; s_chunk[2 * threadIdx.x + 1] = 0;
  u32 stride = gridDim.x * blockDim.x;
  long long c0 = clock64();
  for (u32 i = blockIdx.x * blockDim.x + threadIdx.x; i < count; i += stride) fire_sim(mask, i, variant);
  if (threadIdx.x == 0 && blockIdx.x == 0) *cyc = clock64() - c0;
}

#define CK(x) do { cudaError_t e = (x); if (e != cudaSuccess) { printf("%s: %s\n", #x, cudaGetErrorString(e)); exit(1); } } while (0)

static void reset(Dev &d, u32 nrules, u32 bcap) {
  u32 one = 1;
  u64 one64 = 1;
  CK(cudaMemset(d.nfreen, 0, 4 * MAXLANES));
  CK(cudaMemset(d.nchunk, 0, 8 * MAXLANES));
  CK(cudaMemset(d.nwn, 0, 4 * MAXLANES));
  CK(cudaMemset(d.rfreen, 0, 4 * MAXLANES));
  CK(cudaMemset(d.ovf, 0, 4 * d.ovfcap));
  CK(cudaMemset(d.ovftop, 0, 4));
  CK(cudaMemset(d.blen, 0, 4 * nrules));
  CK(cudaMemset(d.bdone, 0, 4 * nrules));
  CK(cudaMemset(d.abortf, 0, 4));
  CK(cudaMemcpy(d.nbump, &one, 4, cudaMemcpyHostToDevice));
  CK(cudaMemcpy(d.rbump, &one, 4, cudaMemcpyHostToDevice));
  CK(cudaMemcpy(d.hbump, &one64, 8, cudaMemcpyHostToDevice));
  CK(cudaMemcpy(d.labels, &one, 4, cudaMemcpyHostToDevice));
  (void)bcap;
}

int main(int argc, char **argv) {
  u32 ncap = 1u << 26, rcap = 1u << 22, bcap = 1u << 20, ovfcap = 1u << 20, nrules = PROG_NRULES;
  Dev d;
  CK(cudaMalloc(&d.nodes, 16ull * ncap));
  CK(cudaMalloc(&d.rc, 4ull * ncap));
  CK(cudaMalloc(&d.recs, sizeof(Rec) * (size_t)rcap));
  CK(cudaMalloc(&d.nbump, 4));
  CK(cudaMalloc(&d.rbump, 4));
  CK(cudaMalloc(&d.nfree, 4ull * MAXLANES * FREECAP));
  CK(cudaMalloc(&d.nfreen, 4ull * MAXLANES));
  CK(cudaMalloc(&d.nchunk, 8ull * MAXLANES));
  CK(cudaMalloc(&d.ovf, 4ull * ovfcap));
  CK(cudaMalloc(&d.ovftop, 4));
  CK(cudaMalloc(&d.ebuf, 24ull * bcap * nrules));
  CK(cudaMalloc(&d.blen, 4ull * nrules));
  CK(cudaMalloc(&d.bdone, 4ull * nrules));
  CK(cudaMalloc(&d.result, 16));
  CK(cudaMalloc(&d.abortf, 4));
  CK(cudaMalloc(&d.heap, 8ull << 20));
  CK(cudaMalloc(&d.hbump, 8));
  CK(cudaMalloc(&d.nw, 16ull * NWCAP * MAXLANES));
  CK(cudaMalloc(&d.nwn, 4ull * MAXLANES));
  CK(cudaMalloc(&d.labels, 4));
  CK(cudaMalloc(&d.rfree, 4ull * RFREECAP * MAXLANES));
  CK(cudaMalloc(&d.rfreen, 4ull * MAXLANES));
  d.hcap = 1u << 20;
  d.ncap = ncap; d.rcap = rcap; d.bcap = bcap; d.ovfcap = ovfcap; d.chunksz = 1024; d.nrules = nrules;
  d.fuel = 64; d.net_fuel = 4096;
  CK(cudaMemset(d.rc, 0, 4ull * ncap));
  CK(cudaMemcpyToSymbol(G, &d, sizeof(Dev)));
  unsigned long long *cyc;
  CK(cudaMalloc(&cyc, 8));
  cudaEvent_t e0, e1;
  cudaEventCreate(&e0);
  cudaEventCreate(&e1);
  const char *names[] = {"alloc", "free", "read", "rec", "spawn", "deliver"};
  u32 masks[] = {C_ALLOC, C_ALLOC | C_FREE, C_READ, C_REC, C_REC | C_SPAWN, C_REC | C_DELIVER, C_ALL};
  const char *labels[] = {"alloc only", "alloc+free", "read only (synthetic addrs)", "record alloc/free", "record + spawn", "record + deliver", "the whole fire"};
  u32 steps = argc > 1 ? atoi(argv[1]) : 100000;
  u32 variant = argc > 2 ? atoi(argv[2]) : 0;
  printf("allocator variant %u\n", variant);
  printf("%-30s %14s %14s | %14s %12s\n", "component", "1 thread cyc", "1 thread us", "grid cyc/fire", "grid us/fire");
  for (int m = 0; m < 7; m++) {
    reset(d, nrules, bcap);
    CK(cudaDeviceSynchronize());
    cudaEventRecord(e0);
    k_seq<<<1, 1>>>(steps, masks[m], variant, cyc);
    cudaEventRecord(e1);
    CK(cudaDeviceSynchronize());
    unsigned long long c;
    CK(cudaMemcpy(&c, cyc, 8, cudaMemcpyDeviceToHost));
    float ms;
    cudaEventElapsedTime(&ms, e0, e1);
    double seq_cyc = (double)c / steps, seq_us = ms * 1e3 / steps;
    // a wide wave: the same number of fires spread over the full grid
    u32 count = bcap; // one bucket's worth: spawns never overflow
    reset(d, nrules, bcap);
    CK(cudaDeviceSynchronize());
    cudaEventRecord(e0);
    k_par<<<256, 256>>>(count, masks[m], variant, cyc);
    cudaEventRecord(e1);
    CK(cudaDeviceSynchronize());
    cudaEventElapsedTime(&ms, e0, e1);
    CK(cudaMemcpy(&c, cyc, 8, cudaMemcpyDeviceToHost));
    printf("%-30s %14.0f %14.2f | %14.0f %12.4f\n", labels[m], seq_cyc, seq_us, (double)c / (count / 65536.0), ms * 1e3 / count);
  }
  u32 ab;
  CK(cudaMemcpy(&ab, d.abortf, 4, cudaMemcpyDeviceToHost));
  if (ab) printf("abort flag %u\n", ab);
  (void)names;
  return 0;
}
