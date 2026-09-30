// components, C twin: lock-free union-find with CAS. A union links the
// larger root under the smaller one by a CAS on the root's parent word
// (retrying if another thread linked it first); find halves paths with a
// CAS. Parents only point to smaller ids, so every root is its
// component's minimum id. Same graph and checksum as main.py.
// Build: gcc -O2 -fopenmp -pthread main.c
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define LOG 20 // SIZE

static _Atomic uint32_t *parent;

static inline uint32_t prng(uint32_t x) {
  x ^= x << 13;
  x ^= x >> 17;
  x ^= x << 5;
  return x;
}

static inline uint32_t mix(uint32_t i, uint32_t v) {
  return prng(((i + 1u) * 2654435761u) ^ (v * 2246822519u));
}

static inline uint32_t end(uint32_t x) { return prng((x + 1u) * 2654435761u) & ((1u << LOG) - 1u); }

static uint32_t find(uint32_t x) {
  for (;;) {
    uint32_t p = atomic_load(&parent[x]);
    if (p == x) return x;
    uint32_t g = atomic_load(&parent[p]);
    if (g != p) atomic_compare_exchange_weak(&parent[x], &p, g);
    x = p;
  }
}

static void unite(uint32_t a, uint32_t b) {
  for (;;) {
    a = find(a), b = find(b);
    if (a == b) return;
    if (a < b) {
      uint32_t t = a;
      a = b, b = t;
    }
    uint32_t want = a;
    if (atomic_compare_exchange_strong(&parent[a], &want, b)) return;
  }
}

int main(void) {
  const uint32_t n = 1u << LOG;
  parent = malloc(sizeof(_Atomic uint32_t) * n);
  for (uint32_t v = 0; v < n; v++) atomic_init(&parent[v], v);
#pragma omp parallel for schedule(static)
  for (uint32_t e = 0; e < n; e++) unite(end(2 * e), end(2 * e + 1));
  uint32_t comps = 0, h = 0;
  for (uint32_t v = 0; v < n; v++) {
    uint32_t r = find(v);
    comps += r == v;
    h += mix(v, r);
  }
  printf("(%u, %u)\n", comps, h);
  free((void *)parent);
  return 0;
}
