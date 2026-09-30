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
    x = ((10 if (0 if a < a else 6) < (18 if a < 18 else b) else pair(18, b)[0]) if ap(lambda x: x * total(build(1, b)) + 1, pair(a, a)[0]) < a else ((b - b) - 3))
    return total(build(4, pair(0, 4)[0]))


def h1(a, b):
    x = (ap(lambda x: x * total(build(5, 4)) + 0, h0(3, 14)) + (h0(a, b) + (16 - 10)))
    return pair(ap(lambda x: x * a + 8, x), pair(h0(0, x), x)[1])[1]


def h2(a, b):
    x = (ap(lambda x: x * (a if b < b else 3) + 0, (b + 2)) - b)
    return (x if ap(lambda x: x * h1(b, 2) + 7, a) < pair(b, (a if 1 < x else 13))[0] else ((2 + 16) ^ total(build(1, a))))


def main():
    y = ap(lambda x: x * (pair(8, 2)[0] - 1) + 1, 4)
    f = lambda x: x * (1 if h1(6, y) < (y if 16 < y else y) else (y * 4)) + y
    l = build(2, y)
    return (y, pair(ap(lambda x: x * h0(19, y) + 4, 7), y)[0], f(y) + f(y), total(l) + total(l))
