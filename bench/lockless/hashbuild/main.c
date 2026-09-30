// hashbuild, C twin: 2^S keys into a shared chained table of 2^S slots,
// guarded by 4096 striped pthread mutexes (a slot's stripe is its low 12
// bits). An insert locks the stripe, walks the chain and pushes the key if
// absent. Same keys, slots and checksum as main.py.
// Build: gcc -O2 -fopenmp -pthread main.c
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define LOG 20 // SIZE
#define STRIPES 4096u

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

static pthread_mutex_t locks[STRIPES];

int main(void) {
  const int64_t n = (int64_t)1 << LOG;
  int32_t *head = malloc(sizeof(int32_t) * n);
  int32_t *next = malloc(sizeof(int32_t) * n);
  uint32_t *keys = malloc(sizeof(uint32_t) * n);
  for (int64_t s = 0; s < n; s++) head[s] = -1;
  for (uint32_t i = 0; i < STRIPES; i++) pthread_mutex_init(&locks[i], NULL);
#pragma omp parallel for schedule(static)
  for (int64_t i = 0; i < n; i++) {
    uint32_t k = key((uint32_t)i), s = slot(k);
    pthread_mutex_t *m = &locks[s & (STRIPES - 1)];
    pthread_mutex_lock(m);
    int32_t e = head[s];
    while (e >= 0 && keys[e] != k) e = next[e];
    if (e < 0) {
      keys[i] = k;
      next[i] = head[s];
      head[s] = (int32_t)i;
    }
    pthread_mutex_unlock(m);
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
  free(head), free(next), free(keys);
  return 0;
}
