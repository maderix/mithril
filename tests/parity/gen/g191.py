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
    x = 0
    return (16 ^ 6)


def h1(a, b):
    x = (((a if 14 < 4 else a) if (9 if 2 < 13 else b) < pair(b, 14)[1] else pair(2, 5)[0]) & ap(lambda x: x * (8 if 2 < 14 else a) + 6, total(build(3, a))))
    return ap(lambda x: x * ((1 if 2 < 4 else 13) ^ (x if a < x else 12)) + 1, (ap(lambda x: x * x + 8, 19) if h0(x, x) < 9 else pair(16, b)[0]))


def h2(a, b):
    x = ap(lambda x: x * pair(pair(b, 18)[1], ap(lambda x: x * a + 0, 13))[1] + 3, ((9 if 6 < 5 else 7) - (1 if 19 < a else a)))
    return 1


def main():
    y = 11
    f = lambda x: x * ap(lambda x: x * h2(y, y) + 8, (y + y)) + y
    l = build(2, y)
    return ((y if total(build(5, (16 if 4 < 5 else y))) < total(build(3, (12 - y))) else ap(lambda x: x * 18 + 7, y)), pair((y if ap(lambda x: x * y + 6, 16) < pair(19, 13)[0] else y), (y if y < total(build(1, y)) else y))[0], f((y if total(build(5, (16 if 4 < 5 else y))) < total(build(3, (12 - y))) else ap(lambda x: x * 18 + 7, y))) + f(y), total(l) + total(l))
