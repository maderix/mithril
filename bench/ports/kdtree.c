// C twin of kdtree.py (same hashes, same tree, same query order).
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef uint32_t u32;
typedef int64_t i64;
static u32 prng(u32 x) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; return x; }
static u32 px(u32 i) { return prng((i + 1) * 2654435761u) & 65535; }
static u32 py(u32 i) { return prng((i + 1) * 40503u) & 65535; }
typedef struct P { i64 x, y, i; struct P *rest; } P;
typedef struct T { int kind; i64 axis, mid, x, y, i; struct T *lo, *hi; P *ps; } T;
static P *cons(i64 x, i64 y, i64 i, P *r) { P *p = malloc(sizeof *p); p->x = x; p->y = y; p->i = i; p->rest = r; return p; }
static T *build(P *ps, int axis, i64 x0, i64 y0, i64 wx, i64 wy) {
  T *t = malloc(sizeof *t);
  if (!ps) { t->kind = 0; return t; }
  if (!ps->rest) { t->kind = 1; t->x = ps->x; t->y = ps->y; t->i = ps->i; return t; }
  if (wx == 1 && wy == 1) { t->kind = 3; t->ps = ps; return t; }
  int ax = axis;
  if ((axis == 0 && wx == 1) || (axis == 1 && wy == 1)) ax = 1 - axis;
  i64 h = ax == 0 ? wx >> 1 : wy >> 1, mid = (ax == 0 ? x0 : y0) + h;
  P *lo = 0, *hi = 0;
  for (P *p = ps; p; p = p->rest) { i64 c = ax == 0 ? p->x : p->y; if (c < mid) lo = cons(p->x, p->y, p->i, lo); else hi = cons(p->x, p->y, p->i, hi); }
  t->kind = 2; t->axis = ax; t->mid = mid;
  if (ax == 0) { t->lo = build(lo, 1, x0, y0, h, wy); t->hi = build(hi, 1, x0 + h, y0, wx - h, wy); }
  else { t->lo = build(lo, 0, x0, y0, wx, h); t->hi = build(hi, 0, x0, y0 + h, wx, wy - h); }
  return t;
}
static void nearest(T *t, i64 qx, i64 qy, i64 *bd, i64 *bi) {
  if (t->kind == 0) return;
  if (t->kind == 1) { i64 dx = qx - t->x, dy = qy - t->y, d = dx * dx + dy * dy; if (d < *bd) { *bd = d; *bi = t->i; } return; }
  if (t->kind == 3) { for (P *p = t->ps; p; p = p->rest) { i64 dx = qx - p->x, dy = qy - p->y, d = dx * dx + dy * dy; if (d < *bd) { *bd = d; *bi = p->i; } } return; }
  i64 q = t->axis == 0 ? qx : qy, s = q - t->mid;
  if (s < 0) { nearest(t->lo, qx, qy, bd, bi); if (s * s < *bd) nearest(t->hi, qx, qy, bd, bi); }
  else { nearest(t->hi, qx, qy, bd, bi); if (s * s < *bd) nearest(t->lo, qx, qy, bd, bi); }
}
int main(int argc, char **argv) {
  int n = argc > 1 ? atoi(argv[1]) : 18, m = argc > 2 ? atoi(argv[2]) : 18;
  P *ps = 0;
  for (i64 i = (1 << n) - 1; i >= 0; i--) ps = cons(px(i), py(i), i, ps);
  T *t = build(ps, 0, 0, 0, 65536, 65536);
  u32 sum = 0;
  for (i64 j = 0; j < (1 << m); j++) {
    i64 qx = prng((u32)((j + 7) * 2246822519u)) & 65535, qy = prng((u32)((j + 7) * 3266489917u)) & 65535;
    i64 bd = 4294967295LL, bi = 0;
    nearest(t, qx, qy, &bd, &bi);
    sum += (u32)((bi * 2654435761LL + bd) & 4294967295LL);
  }
  printf("%u\n", sum);
  return 0;
}
