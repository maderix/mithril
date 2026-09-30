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
    x = ((13 if 12 < 14 else total(build(6, 8))) if a < total(build(2, pair(12, a)[0])) else pair(16, ap(lambda x: x * 16 + 3, b))[1])
    return 17


def h1(a, b):
    x = (pair(pair(7, a)[1], a)[0] if total(build(5, pair(a, a)[1])) < ap(lambda x: x * a + 5, (2 if 5 < a else 14)) else ((11 - 15) + (b - 10)))
    return a


def h2(a, b):
    x = (ap(lambda x: x * b + 1, 10) + pair((17 if 19 < 2 else a), ap(lambda x: x * a + 4, 19))[0])
    return (pair(h0(4, a), (13 if 10 < 10 else x))[1] if ((3 if 3 < x else 6) if pair(a, 6)[0] < a else h0(b, 4)) < 1 else (total(build(2, x)) if a < (x if b < a else b) else h1(10, 0)))


def main():
    y = ap(lambda x: x * ap(lambda x: x * h2(5, 5) + 6, ap(lambda x: x * 9 + 5, 7)) + 1, 5)
    f = lambda x: x * (total(build(4, 11)) & 14) + y
    l = build(3, y)
    return (h0(ap(lambda x: x * h0(11, 8) + 5, (11 & y)), pair((y if 16 < 17 else 7), 17)[0]), pair((total(build(2, 14)) if pair(y, y)[0] < total(build(5, y)) else ap(lambda x: x * y + 6, y)), pair((1 * 18), ap(lambda x: x * 5 + 5, y))[0])[0], f(h0(ap(lambda x: x * h0(11, 8) + 5, (11 & y)), pair((y if 16 < 17 else 7), 17)[0])) + f(y), total(l) + total(l))
