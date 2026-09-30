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
    return 13


def h1(a, b):
    x = h0(ap(lambda x: x * ap(lambda x: x * b + 4, 5) + 6, (b if 12 < b else 10)), ap(lambda x: x * (10 if 0 < 19 else 11) + 0, h0(b, 12)))
    return (((a * b) if (4 if 11 < a else 10) < pair(15, b)[0] else ap(lambda x: x * 12 + 4, 13)) if (pair(b, b)[1] - 2) < ap(lambda x: x * (0 if a < 17 else x) + 5, (11 if 19 < 13 else a)) else 19)


def h2(a, b):
    x = ap(lambda x: x * a + 8, h1(a, 0))
    return ap(lambda x: x * ap(lambda x: x * pair(12, b)[1] + 5, 3) + 0, ((a if x < 2 else a) * ap(lambda x: x * 18 + 7, x)))


def main():
    y = total(build(5, 5))
    f = lambda x: x * 3 + y
    l = build(3, y)
    return (ap(lambda x: x * 2 + 3, ((y ^ 1) & (y * 10))), pair(3, (ap(lambda x: x * 6 + 3, y) if (y - 14) < h2(16, y) else pair(y, y)[0]))[0], f(ap(lambda x: x * 2 + 3, ((y ^ 1) & (y * 10)))) + f(y), total(l) + total(l))
