// bfs, C twin: level-synchronous BFS from node 0; a node is claimed for
// the next level by a CAS on its level word, and the claimant appends it
// to the next frontier through an atomic tail counter. Same graph and
// checksum as main.py. Build: gcc -O2 -fopenmp -pthread main.c
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define LOG 20 // SIZE
#define UNSEEN 0xFFFFFFFFu

static inline uint32_t prng(uint32_t x) {
  x ^= x << 13;
  x ^= x >> 17;
  x ^= x << 5;
  return x;
}

static inline uint32_t nbr(uint32_t v, uint32_t j) {
  return prng(((v * 8u + j + 1u) * 2654435761u) ^ 2246822519u) & ((1u << LOG) - 1u);
}

int main(void) {
  const uint32_t n = 1u << LOG;
  _Atomic uint32_t *level = malloc(sizeof(_Atomic uint32_t) * n);
  uint32_t *cur = malloc(sizeof(uint32_t) * n), *nxt = malloc(sizeof(uint32_t) * n);
  for (uint32_t v = 0; v < n; v++) atomic_init(&level[v], UNSEEN);
  atomic_store(&level[0], 0);
  cur[0] = 0;
  uint32_t clen = 1, d = 0, reached = 0;
  uint64_t acc = 0;
  _Atomic uint32_t tail;
  while (clen > 0) {
    reached += clen;
    acc += (uint64_t)d * clen;
    atomic_store(&tail, 0);
#pragma omp parallel for schedule(dynamic, 256)
    for (uint32_t i = 0; i < clen; i++) {
      uint32_t v = cur[i];
      for (uint32_t j = 0; j < 8; j++) {
        uint32_t u = nbr(v, j), want = UNSEEN;
        if (atomic_load_explicit(&level[u], memory_order_relaxed) == UNSEEN &&
            atomic_compare_exchange_strong(&level[u], &want, d + 1))
          nxt[atomic_fetch_add(&tail, 1)] = u;
      }
    }
    uint32_t *t = cur;
    cur = nxt, nxt = t;
    clen = atomic_load(&tail);
    d++;
  }
  printf("(%u, %llu)\n", reached, (unsigned long long)acc);
  free((void *)level), free(cur), free(nxt);
  return 0;
}
