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
    x = total(build(2, 2))
    return pair((10 if (3 if 12 < 2 else x) < ap(lambda x: x * 7 + 6, b) else 18), (19 * pair(0, 6)[1]))[0]


def h1(a, b):
    x = (a if (pair(10, 14)[1] & (b if b < a else 7)) < h0(pair(a, 0)[0], pair(b, 14)[1]) else (b if (a - 13) < total(build(1, a)) else (a if a < 10 else a)))
    return ap(lambda x: x * (b if (b if 8 < 9 else a) < a else ap(lambda x: x * b + 5, 17)) + 7, b)


def h2(a, b):
    x = b
    return total(build(5, pair((9 * a), 5)[1]))


def main():
    y = (((15 if 15 < 18 else 7) if pair(1, 1)[0] < total(build(5, 13)) else pair(7, 9)[1]) if total(build(5, (12 ^ 6))) < (2 * h1(9, 10)) else ((5 - 1) if total(build(3, 0)) < 11 else total(build(4, 2))))
    f = lambda x: x * y + y
    l = build(1, y)
    return (0, ((ap(lambda x: x * y + 2, 19) + y) * 7), f(0) + f(y), total(l) + total(l))
