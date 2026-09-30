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
    x = total(build(4, pair(pair(1, 13)[1], (1 if a < b else a))[0]))
    return ap(lambda x: x * 16 + 4, pair(a, pair(7, b)[0])[0])


def h1(a, b):
    x = total(build(5, 11))
    return (x if (h0(5, x) - 18) < pair(h0(a, 16), pair(18, 19)[1])[0] else a)


def h2(a, b):
    x = (a if ((4 - 11) if ap(lambda x: x * 0 + 2, a) < a else pair(2, a)[1]) < a else ap(lambda x: x * 17 + 0, (a & b)))
    return ((h1(2, 0) if (a * b) < a else a) if ((4 ^ 17) if pair(b, b)[0] < total(build(0, 7)) else total(build(2, 9))) < (total(build(6, 8)) if (b * b) < ap(lambda x: x * x + 0, 0) else (0 * 2)) else ap(lambda x: x * 1 + 7, (16 if x < 9 else b)))


def main():
    y = ((ap(lambda x: x * 17 + 0, 2) if ap(lambda x: x * 6 + 0, 1) < (3 if 6 < 2 else 15) else (8 if 1 < 7 else 12)) if (h1(13, 16) if 9 < h0(19, 15) else 0) < h0(ap(lambda x: x * 17 + 1, 10), pair(2, 13)[1]) else 2)
    f = lambda x: x * y + y
    l = build(2, y)
    return ((14 + 12), 10, f((14 + 12)) + f(y), total(l) + total(l))
