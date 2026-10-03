// C twin of heat2d.py (same start grid, same stencil, same border rule).
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef uint32_t u32;
static u32 prng(u32 x) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x; }
int main(void) {
  const int64_t w = 1024, steps = 500, n = w * w;
  int64_t *g = malloc(n * sizeof *g), *ng = malloc(n * sizeof *ng);
  for (int64_t i = 0; i < n; i++) g[i] = (int64_t)(prng((u32)((i + 1) * 2654435761u)) & 1023) * 256;
  for (int64_t t = 0; t < steps; t++) {
    for (int64_t i = 0; i < n; i++) {
      int64_t x = i % w, y = i / w;
      if (x == 0 || y == 0 || x == w - 1 || y == w - 1) ng[i] = g[i];
      else ng[i] = (4 * g[i] + g[i - w] + g[i + w] + g[i - 1] + g[i + 1]) / 8;
    }
    int64_t *tmp = g; g = ng; ng = tmp;
  }
  u32 s = 0;
  for (int64_t i = 0; i < n; i++) s += (u32)g[i];
  printf("%u\n", s);
  return 0;
}
