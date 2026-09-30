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
    x = ap(lambda x: x * b + 6, total(build(6, ap(lambda x: x * a + 0, a))))
    return 6


def h1(a, b):
    x = ((total(build(3, 19)) if (9 & a) < total(build(2, a)) else (b & 9)) if pair(ap(lambda x: x * 5 + 0, 6), h0(13, 16))[1] < b else ap(lambda x: x * pair(b, a)[1] + 0, a))
    return ap(lambda x: x * total(build(2, (x if 0 < x else 16))) + 3, pair(14, 4)[0])


def h2(a, b):
    x = h1(b, 14)
    return ((h1(3, a) if total(build(6, b)) < x else b) & ap(lambda x: x * ap(lambda x: x * a + 0, x) + 2, x))


def main():
    y = 15
    f = lambda x: x * 8 + y
    l = build(0, y)
    return (pair(ap(lambda x: x * y + 3, total(build(6, 6))), 16)[0], ap(lambda x: x * h0(pair(y, 0)[0], y) + 6, h1(ap(lambda x: x * y + 6, y), 5)), f(pair(ap(lambda x: x * y + 3, total(build(6, 6))), 16)[0]) + f(y), total(l) + total(l))
