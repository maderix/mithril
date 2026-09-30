// fsum, C fixed-shape tree twin: the same f32 sum as main.py, grouped the
// same way. The range is cut into 2^P aligned chunks (P = min(S, 8)); each
// chunk is summed by the recursive halving tree (OpenMP parallel for over
// the chunks, each partial in its own slot), then the 2^P partials are
// added by the same halving tree. The grouping depends only on S, so the
// bits are the same at every thread count and equal Mithril's. It shows
// that main.c's schedule dependence comes from its reduction's shape, not
// from C. Build: gcc -O2 -fopenmp main_tree.c (no -ffast-math).
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define LOG 24 // SIZE
#define P (LOG < 8 ? LOG : 8)

static inline uint32_t prng(uint32_t x) {
  x ^= x << 13;
  x ^= x >> 17;
  x ^= x << 5;
  return x;
}

static inline float val(uint32_t i) {
  uint32_t h = prng((i + 1u) * 2654435761u);
  return (float)(h & 16777215u) / 16777216.0f - 0.5f;
}

// the halving tree over 2^k values from lo (tsum in main.py)
static float tsum(uint32_t lo, int k) {
  if (k == 0) return val(lo);
  return tsum(lo, k - 1) + tsum(lo + (1u << (k - 1)), k - 1);
}

// the same tree over 2^k stored partials
static float psum(const float *a, int k) {
  if (k == 0) return a[0];
  return psum(a, k - 1) + psum(a + ((int64_t)1 << (k - 1)), k - 1);
}

int main(void) {
  const int64_t chunks = (int64_t)1 << P;
  float *part = malloc(sizeof(float) * chunks);
#pragma omp parallel for schedule(static)
  for (int64_t c = 0; c < chunks; c++) part[c] = tsum((uint32_t)(c << (LOG - P)), LOG - P);
  float s = psum(part, P);
  uint32_t bits;
  memcpy(&bits, &s, sizeof bits);
  printf("%u\n", bits);
  free(part);
  return 0;
}
