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
    x = ap(lambda x: x * 14 + 3, 13)
    return (4 + (ap(lambda x: x * b + 7, x) if ap(lambda x: x * a + 6, 8) < (a + 10) else a))


def h1(a, b):
    x = (total(build(2, 6)) * b)
    return x


def h2(a, b):
    x = (h0(b, (a if 7 < 4 else a)) if (pair(19, b)[0] if h1(14, 0) < (b & 8) else b) < ap(lambda x: x * ap(lambda x: x * 1 + 6, 10) + 0, b) else ap(lambda x: x * (7 * 11) + 7, pair(a, b)[1]))
    return a


def main():
    y = h2(((6 ^ 9) & (13 if 5 < 17 else 10)), (11 & 1))
    f = lambda x: x * (total(build(1, 19)) if pair(2, 4)[1] < ap(lambda x: x * y + 7, y) else h1(y, 16)) + y
    l = build(5, y)
    return ((ap(lambda x: x * ap(lambda x: x * y + 6, y) + 2, total(build(6, y))) if 3 < total(build(4, y)) else (8 * h2(19, y))), total(build(4, total(build(1, h1(18, 4))))), f((ap(lambda x: x * ap(lambda x: x * y + 6, y) + 2, total(build(6, y))) if 3 < total(build(4, y)) else (8 * h2(19, y)))) + f(y), total(l) + total(l))
