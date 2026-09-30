// fsum, C twin: the f32 sum of 2^S values with an OpenMP reduction. Each
// thread sums its share in order and the partial sums are combined in the
// order the threads arrive, so the rounding (and the printed bits) can
// change with the thread count and from run to run. Same values as
// main.py; prints the sum's bit pattern. Build: gcc -O2 -fopenmp main.c
// (no -ffast-math: every addition is a rounded binary32 add).
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define LOG 24 // SIZE

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

int main(void) {
  const int64_t n = (int64_t)1 << LOG;
  float s = 0.0f;
#pragma omp parallel for reduction(+ : s)
  for (int64_t i = 0; i < n; i++) s += val((uint32_t)i);
  uint32_t bits;
  memcpy(&bits, &s, sizeof bits);
  printf("%u\n", bits);
  return 0;
}
