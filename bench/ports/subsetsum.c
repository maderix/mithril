// C twin of subsetsum.py (same weights, same capacity, same count).
#include <stdio.h>
#include <stdint.h>
typedef uint32_t u32;
static u32 prng(u32 x) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x; }
static int64_t weight(int64_t i) { return (prng((u32)((i + 1) * 2654435761u)) & 65535) + 1; }
static u32 count(int64_t i, int64_t n, int64_t room) {
  if (i == n) return 1;
  u32 skip = count(i + 1, n, room);
  int64_t w = weight(i);
  if (w > room) return skip;
  return skip + count(i + 1, n, room - w);
}
int main(void) {
  int64_t n = 32, s = 0;
  for (int64_t i = 0; i < n; i++) s += weight(i);
  printf("%u\n", count(0, n, s / 3));
  return 0;
}
