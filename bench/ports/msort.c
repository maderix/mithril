// C twin of msort.py (same list, same split order, same merge, same hash).
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef struct L { int64_t h; struct L *t; } L;
static L *cons(int64_t h, L *t) { L *c = malloc(sizeof *c); c->h = h; c->t = t; return c; }
static L *merge(L *a, L *b) {
  L head, *tail = &head;
  while (a && b) { if (a->h <= b->h) { tail->t = cons(a->h, 0); a = a->t; } else { tail->t = cons(b->h, 0); b = b->t; } tail = tail->t; }
  tail->t = a ? a : b;
  return head.t;
}
static L *msort(L *l) {
  if (!l || !l->t) return l;
  L *a = 0, *b = 0;
  for (; l; l = l->t) { L *n = cons(l->h, a); a = b; b = n; }
  return merge(msort(a), msort(b));
}
int main(void) {
  L *l = 0; int64_t x = 1;
  for (int64_t n = 524288; n > 0; n--) { x = (x * 1103515245 + 12345) & 2147483647; l = cons(x & 1048575, l); }
  l = msort(l);
  uint32_t acc = 0; int64_t i = 1;
  for (; l; l = l->t, i++) acc = (uint32_t)((acc * 31ull + (uint64_t)(l->h * i)) & 4294967295u);
  printf("%u\n", acc);
  return 0;
}
