// C twin of knapsack.py (same items, same capacity, same table).
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef uint32_t u32;
static u32 prng(u32 x) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x; }
int main(void) {
  const int64_t items = 8000, cap = 100000;
  int64_t *row = calloc(cap + 1, sizeof *row), *nr = calloc(cap + 1, sizeof *nr);
  for (int64_t i = 0; i < items; i++) {
    int64_t w = (prng((u32)((i + 1) * 2654435761u)) & 2047) + 1, v = (prng((u32)((i + 1) * 40503u)) & 4095) + 1;
    for (int64_t c = 0; c <= cap; c++) { int64_t keep = row[c]; nr[c] = (c >= w && row[c - w] + v > keep) ? row[c - w] + v : keep; }
    int64_t *tmp = row; row = nr; nr = tmp;
  }
  printf("%u\n", (u32)row[cap]);
  return 0;
}
