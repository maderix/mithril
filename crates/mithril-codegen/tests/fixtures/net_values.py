# Values built by the net (closure results, tuples made in a closure body)
# reaching compiled code: every field must be settled before compiled
# code reads it. A closure returning a tuple through a self-recursive
# applier, a closure chosen at runtime, nested tuples of floats, a tuple
# argument built in a closure body passed to a compiled function, and a
# result consumed by arithmetic.
def ap(f, s, n):
    if n == 0:
        return f(s)
    return ap(f, s, n - 1)


def fst(p, n):
    if n == 0:
        return p[0] * 2.0
    return fst(p, n - 1)


def main():
    n = array_len(array_new(3, 0))
    s = array_get(array_new(n, 2.5), 0)
    pair = ap(lambda y: (y, y), n, n)
    sum2 = pair[0] + pair[1]
    f = lambda y: (y, y)
    g = lambda y: (y + 1, 0)
    h = f if n > 1 else g
    nested = ap(lambda y: (y * 1.5, (y * 2.0, 0)), 2.0, n)
    inner = nested[1]
    through = ap(lambda y: fst((y, 1), n), s, n)
    return (pair, sum2, h(n), nested[0] + inner[0], inner[1], through)
