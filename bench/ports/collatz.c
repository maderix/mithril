// C twin of collatz.py (same range, same u32 total).
#include <stdio.h>
#include <stdint.h>
static int64_t steps(int64_t n) { int64_t c = 0; while (n != 1) { n = (n % 2 == 0) ? n / 2 : 3 * n + 1; c++; } return c; }
int main(void) {
  uint32_t s = 0;
  for (int64_t i = 1; i < 3000000; i++) s += (uint32_t)steps(i);
  printf("%u\n", s);
  return 0;
}
