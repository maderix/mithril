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
    x = pair(((b + 14) - 7), ap(lambda x: x * total(build(0, b)) + 1, 0))[0]
    return pair(3, ap(lambda x: x * (16 & x) + 4, (16 * 3)))[0]


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * (a if 11 < 3 else 11) + 1, 5) + 1, (h0(a, b) - pair(9, 17)[1]))
    return ap(lambda x: x * 11 + 8, h0(11, ap(lambda x: x * b + 6, 16)))


def h2(a, b):
    x = (pair((a & b), ap(lambda x: x * b + 7, a))[0] * ((a if 12 < b else 4) if 6 < pair(8, 6)[1] else (3 if 17 < b else 5)))
    return pair(pair(pair(x, b)[0], h0(10, b))[1], ap(lambda x: x * a + 6, a))[0]


def main():
    y = 2
    f = lambda x: x * ap(lambda x: x * h0(3, y) + 7, (y & y)) + y
    l = build(0, y)
    return ((((y if y < 14 else y) ^ (y if y < 11 else 6)) if ap(lambda x: x * (y if 17 < 19 else y) + 7, (y - y)) < (total(build(0, 10)) if ap(lambda x: x * 1 + 1, y) < ap(lambda x: x * 5 + 5, y) else 11) else y), total(build(1, ap(lambda x: x * y + 0, (10 ^ y)))), f((((y if y < 14 else y) ^ (y if y < 11 else 6)) if ap(lambda x: x * (y if 17 < 19 else y) + 7, (y - y)) < (total(build(0, 10)) if ap(lambda x: x * 1 + 1, y) < ap(lambda x: x * 5 + 5, y) else 11) else y)) + f(y), total(l) + total(l))
