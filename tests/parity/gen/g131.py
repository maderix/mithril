@data
class L:
    Nil: ()
    Cons: (h, t)


def ap(f, v):
    return f(v)


def pair(a, b):
    return (a, b)


def build(n, s):
    if n == 0:
        return Nil()
    return Cons(s + n, build(n - 1, s))


def total(l):
    match l:
        case Nil():
            return 0
        case Cons(h, t):
            return h + total(t)


def h0(a, b):
    x = (pair(pair(b, b)[0], total(build(6, b)))[1] & ap(lambda x: x * 17 + 2, total(build(2, a))))
    return (ap(lambda x: x * (1 if x < x else 1) + 4, 14) if 11 < 7 else 15)


def h1(a, b):
    x = ap(lambda x: x * 2 + 1, a)
    return h0(((4 if 11 < 10 else a) * pair(x, 9)[1]), pair(h0(1, b), x)[1])


def h2(a, b):
    x = ap(lambda x: x * 7 + 1, h1(total(build(4, b)), a))
    return (h1((3 & 1), ap(lambda x: x * x + 6, 3)) if pair((12 - a), (11 * 12))[1] < (total(build(5, 15)) ^ pair(6, 13)[1]) else ((19 ^ 4) * h0(a, 2)))


def main():
    y = (pair((15 if 15 < 19 else 5), (1 & 16))[0] if pair(ap(lambda x: x * 6 + 6, 19), h0(15, 1))[0] < (0 if total(build(0, 0)) < (4 - 4) else pair(9, 17)[1]) else pair(ap(lambda x: x * 10 + 3, 16), pair(10, 2)[1])[0])
    f = lambda x: x * h0(pair(y, 3)[1], 5) + y
    l = build(2, y)
    return (pair(total(build(4, 14)), ap(lambda x: x * (11 & y) + 1, total(build(6, y))))[1], pair((pair(y, 5)[1] if ap(lambda x: x * y + 6, y) < (18 + 13) else (y * y)), total(build(5, (8 if 7 < 8 else y))))[0], f(pair(total(build(4, 14)), ap(lambda x: x * (11 & y) + 1, total(build(6, y))))[1]) + f(y), total(l) + total(l))
