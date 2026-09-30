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
    x = (pair((a if 10 < a else a), (19 * 15))[0] if (6 & pair(b, 16)[1]) < pair(12, b)[0] else 15)
    return (13 if a < ((0 & 7) if pair(x, x)[1] < (b - 11) else pair(a, a)[0]) else 0)


def h1(a, b):
    x = total(build(5, 15))
    return (b if h0(h0(a, x), x) < x else ((9 if 15 < 12 else x) * ap(lambda x: x * b + 1, 0)))


def h2(a, b):
    x = (total(build(6, (9 ^ a))) if total(build(4, h1(b, 16))) < 1 else total(build(0, pair(14, 15)[0])))
    return (pair(ap(lambda x: x * 9 + 1, 3), (x + a))[1] if h1(total(build(4, 9)), (19 ^ 19)) < total(build(6, b)) else total(build(2, 3)))


def main():
    y = pair(6, pair(pair(16, 9)[0], h0(19, 12))[0])[1]
    f = lambda x: x * ((y + 10) if (16 if 3 < 8 else 18) < total(build(3, 5)) else h1(y, 16)) + y
    l = build(6, y)
    return (pair(ap(lambda x: x * ap(lambda x: x * 1 + 6, y) + 0, ap(lambda x: x * y + 1, 13)), 15)[0], pair((total(build(2, y)) if 8 < pair(8, y)[0] else 16), 0)[0], f(pair(ap(lambda x: x * ap(lambda x: x * 1 + 6, y) + 0, ap(lambda x: x * y + 1, 13)), 15)[0]) + f(y), total(l) + total(l))
