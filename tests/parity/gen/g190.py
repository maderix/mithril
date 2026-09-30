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
    x = 4
    return ((total(build(5, a)) if 6 < pair(3, b)[1] else 14) - 13)


def h1(a, b):
    x = h0(b, (pair(5, a)[0] if (b if b < 18 else b) < pair(a, 14)[1] else h0(b, b)))
    return 7


def h2(a, b):
    x = h0(pair((b + 16), (b + 1))[1], ap(lambda x: x * pair(12, b)[1] + 8, (a - 14)))
    return ap(lambda x: x * (19 + h1(b, b)) + 1, 7)


def main():
    y = (total(build(3, pair(2, 9)[0])) * ap(lambda x: x * h2(2, 2) + 4, pair(9, 4)[0]))
    f = lambda x: x * ap(lambda x: x * h0(11, y) + 0, y) + y
    l = build(1, y)
    return (ap(lambda x: x * (pair(16, y)[1] if pair(4, y)[0] < ap(lambda x: x * 7 + 6, 13) else pair(y, y)[1]) + 1, pair((16 if 18 < y else y), y)[1]), ap(lambda x: x * h2((y & 10), h0(y, 15)) + 4, pair(ap(lambda x: x * 4 + 8, 7), (11 if 11 < y else 6))[0]), f(ap(lambda x: x * (pair(16, y)[1] if pair(4, y)[0] < ap(lambda x: x * 7 + 6, 13) else pair(y, y)[1]) + 1, pair((16 if 18 < y else y), y)[1])) + f(y), total(l) + total(l))
