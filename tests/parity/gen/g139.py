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
    x = b
    return pair(ap(lambda x: x * 2 + 6, 8), 5)[0]


def h1(a, b):
    x = ap(lambda x: x * total(build(2, (18 if 14 < 6 else b))) + 6, (a if pair(3, 5)[0] < (11 if b < a else 10) else total(build(4, 4))))
    return a


def h2(a, b):
    x = h1(pair((8 if 17 < 13 else 14), pair(0, a)[1])[1], ap(lambda x: x * h0(16, a) + 6, (8 + 8)))
    return pair(14, ap(lambda x: x * ap(lambda x: x * 8 + 7, b) + 3, h1(b, b)))[0]


def main():
    y = ap(lambda x: x * total(build(0, (7 ^ 6))) + 6, (total(build(2, 3)) if (15 if 2 < 7 else 2) < total(build(6, 6)) else ap(lambda x: x * 15 + 8, 4)))
    f = lambda x: x * h0(total(build(1, y)), pair(16, 7)[0]) + y
    l = build(4, y)
    return (ap(lambda x: x * ap(lambda x: x * (y ^ y) + 7, h2(y, y)) + 3, pair(y, (6 if y < 17 else y))[0]), pair(6, 18)[1], f(ap(lambda x: x * ap(lambda x: x * (y ^ y) + 7, h2(y, y)) + 3, pair(y, (6 if y < 17 else y))[0])) + f(y), total(l) + total(l))
