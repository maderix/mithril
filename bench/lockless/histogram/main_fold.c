// histogram, C fold/merge twin: 2^S keys into 2^16 bins with no shared
// counter. Each thread counts its share of the keys into a private copy of
// the bins (OpenMP array-section reduction), and the private copies are
// added into the result when the loop ends. Same keys and checksum as
// main.py and main.c. Build: gcc -O2 -fopenmp -pthread main_fold.c
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define LOG 24 // SIZE
#define BINS 65536u

static inline uint32_t prng(uint32_t x) {
  x ^= x << 13;
  x ^= x >> 17;
  x ^= x << 5;
  return x;
}

static inline uint32_t mix(uint32_t i, uint32_t v) {
  return prng(((i + 1u) * 2654435761u) ^ (v * 2246822519u));
}

static inline uint32_t key(uint32_t i) { return prng((i + 1u) * 2654435761u) >> 16; }

int main(void) {
  const int64_t n = (int64_t)1 << LOG;
  uint32_t *bins = calloc(BINS, sizeof *bins);
#pragma omp parallel for schedule(static) reduction(+ : bins[0:BINS])
  for (int64_t i = 0; i < n; i++) bins[key((uint32_t)i)]++;
  uint32_t h = 0;
  for (uint32_t i = 0; i < BINS; i++) h += mix(i, bins[i]);
  printf("%u\n", h);
  free(bins);
  return 0;
}
