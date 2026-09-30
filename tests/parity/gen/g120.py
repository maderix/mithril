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
    x = (ap(lambda x: x * (1 if b < 8 else 17) + 6, (b - 15)) * a)
    return ((ap(lambda x: x * 19 + 6, 1) if (b + 17) < ap(lambda x: x * x + 3, 9) else ap(lambda x: x * 0 + 8, 18)) & 16)


def h1(a, b):
    x = (((18 * b) ^ ap(lambda x: x * a + 4, 17)) * ap(lambda x: x * (0 ^ 16) + 7, 12))
    return h0((pair(6, x)[1] * x), total(build(1, 17)))


def h2(a, b):
    x = ap(lambda x: x * 16 + 7, ap(lambda x: x * 19 + 6, total(build(3, b))))
    return (7 if total(build(0, h1(4, b))) < ((a & x) if (a - 13) < pair(a, 5)[0] else 8) else (3 ^ x))


def main():
    y = h2(((13 & 15) if ap(lambda x: x * 13 + 6, 12) < 10 else (7 if 15 < 16 else 6)), 17)
    f = lambda x: x * (h0(y, 19) if ap(lambda x: x * y + 4, 13) < y else y) + y
    l = build(0, y)
    return (y, h1(h1(y, (11 if 5 < y else 13)), pair(11, (18 - y))[0]), f(y) + f(y), total(l) + total(l))
