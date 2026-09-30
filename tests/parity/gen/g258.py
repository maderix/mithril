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
    x = a
    return ap(lambda x: x * (a ^ pair(a, b)[0]) + 7, pair(pair(a, b)[0], (13 if a < 12 else 19))[1])


def h1(a, b):
    x = ap(lambda x: x * h0(pair(13, b)[0], ap(lambda x: x * 8 + 7, 4)) + 2, (pair(3, 4)[1] + (1 if 16 < a else 18)))
    return 0


def h2(a, b):
    x = 9
    return (x ^ 16)


def main():
    y = ap(lambda x: x * ((5 * 9) if (18 if 2 < 8 else 0) < ap(lambda x: x * 11 + 8, 4) else 11) + 6, ap(lambda x: x * h2(12, 19) + 5, 1))
    f = lambda x: x * y + y
    l = build(4, y)
    return (pair(ap(lambda x: x * total(build(3, y)) + 1, (19 if y < y else y)), pair(h1(13, 4), total(build(1, y)))[1])[0], ap(lambda x: x * ap(lambda x: x * h1(y, y) + 1, (19 if y < 7 else y)) + 8, y), f(pair(ap(lambda x: x * total(build(3, y)) + 1, (19 if y < y else y)), pair(h1(13, 4), total(build(1, y)))[1])[0]) + f(y), total(l) + total(l))
