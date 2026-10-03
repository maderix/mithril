// C twin of histogram.py (same keys, same buckets, same checksum).
#include <stdio.h>
#include <stdint.h>
typedef uint32_t u32;
static u32 prng(u32 x) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x; }
int main(void) {
  int64_t h[8] = {0};
  for (int64_t i = 0; i < 536870912; i++) h[prng((u32)((i + 1) * 2654435761u)) & 7]++;
  int64_t s = 0;
  for (int k = 0; k < 8; k++) s += (k + 1) * h[k];
  printf("%u\n", (u32)s);
  return 0;
}
