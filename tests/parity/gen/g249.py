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
    x = ap(lambda x: x * a + 3, (17 if pair(b, 0)[0] < pair(a, a)[0] else pair(a, 7)[0]))
    return x


def h1(a, b):
    x = 17
    return pair(a, pair((b - 19), 18)[1])[0]


def h2(a, b):
    x = a
    return (a if (h0(x, b) if total(build(3, x)) < (17 + x) else pair(b, b)[1]) < (pair(b, 6)[0] * ap(lambda x: x * b + 0, a)) else ((3 if 13 < a else 13) if ap(lambda x: x * a + 1, a) < total(build(3, 13)) else pair(2, 3)[1]))


def main():
    y = pair((2 if 16 < (15 * 12) else pair(13, 7)[0]), ap(lambda x: x * (10 + 5) + 2, total(build(6, 18))))[0]
    f = lambda x: x * (pair(4, 10)[0] if total(build(1, y)) < (y - 1) else 1) + y
    l = build(5, y)
    return (pair(h0((y if y < y else y), (8 & 1)), ((y if 9 < 2 else y) & 2))[0], ap(lambda x: x * (pair(y, y)[0] if h2(7, 8) < pair(y, 3)[0] else y) + 7, pair((y & y), 1)[0]), f(pair(h0((y if y < y else y), (8 & 1)), ((y if 9 < 2 else y) & 2))[0]) + f(y), total(l) + total(l))
