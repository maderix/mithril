// hashbuild, C fold/merge twin: 2^S keys into a chained table of 2^S slots
// with no lock. Phase 1 (after every thread's table is cleared): each thread builds its own chained table (its own
// head array) from its static share of the indices; entry i (keys[i],
// next[i]) is written only by the thread that inserts key i. Phase 2: the
// slots are split among the threads, and the thread owning slot s merges
// every thread's chain for s into one chain, dropping keys already present
// (it relinks only entries of slot s, which no other thread touches). The
// only synchronization is the implicit barrier closing each loop.
// Same keys, slots and checksum as main.py and main.c.
// Build: gcc -O2 -fopenmp -pthread main_fold.c
#include <omp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define LOG 20 // SIZE

static inline uint32_t prng(uint32_t x) {
  x ^= x << 13;
  x ^= x >> 17;
  x ^= x << 5;
  return x;
}

static inline uint32_t mix(uint32_t i, uint32_t v) {
  return prng(((i + 1u) * 2654435761u) ^ (v * 2246822519u));
}

static inline uint32_t key(uint32_t i) {
  return prng(((i + 1u) * 2654435761u) ^ 1779033703u) & ((1u << (LOG + 4)) - 1u);
}

static inline uint32_t slot(uint32_t k) { return (k * 2654435761u) >> (32 - LOG); }

int main(void) {
  const int64_t n = (int64_t)1 << LOG;
  const int nt = omp_get_max_threads();
  int32_t *heads = malloc(sizeof(int32_t) * n * nt); // thread t's table: heads + t * n
  int32_t *head = malloc(sizeof(int32_t) * n);       // the merged table
  int32_t *next = malloc(sizeof(int32_t) * n);
  uint32_t *keys = malloc(sizeof(uint32_t) * n);
#pragma omp parallel for schedule(static)
  for (int64_t j = 0; j < n * nt; j++) heads[j] = -1;
#pragma omp parallel num_threads(nt)
  {
    int32_t *mine = heads + (int64_t)omp_get_thread_num() * n;
#pragma omp for schedule(static)
    for (int64_t i = 0; i < n; i++) {
      uint32_t k = key((uint32_t)i), s = slot(k);
      int32_t e = mine[s];
      while (e >= 0 && keys[e] != k) e = next[e];
      if (e < 0) {
        keys[i] = k;
        next[i] = mine[s];
        mine[s] = (int32_t)i;
      }
    }
  }
#pragma omp parallel for schedule(static)
  for (int64_t s = 0; s < n; s++) {
    int32_t h = -1;
    for (int t = 0; t < nt; t++) {
      for (int32_t e = heads[(int64_t)t * n + s], f; e >= 0; e = f) {
        f = next[e];
        int32_t x = h;
        while (x >= 0 && keys[x] != keys[e]) x = next[x];
        if (x < 0) next[e] = h, h = e;
      }
    }
    head[s] = h;
  }
  uint32_t occ = 0, h = 0;
  for (int64_t s = 0; s < n; s++) {
    if (head[s] < 0) continue;
    uint32_t cnt = 0, sum = 0;
    for (int32_t e = head[s]; e >= 0; e = next[e]) cnt++, sum += keys[e];
    occ++;
    h += mix((uint32_t)s, sum + cnt * 2654435761u);
  }
  printf("(%u, %u)\n", occ, h);
  free(heads), free(head), free(next), free(keys);
  return 0;
}
