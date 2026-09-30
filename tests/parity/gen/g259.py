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
    x = ap(lambda x: x * total(build(4, (6 - a))) + 5, ap(lambda x: x * ap(lambda x: x * 7 + 3, 7) + 5, ap(lambda x: x * 17 + 1, 5)))
    return ap(lambda x: x * 10 + 7, 11)


def h1(a, b):
    x = ap(lambda x: x * a + 1, 17)
    return h0(pair((x if 16 < 15 else a), h0(3, a))[0], total(build(1, (7 if x < b else x))))


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * 17 + 5, total(build(6, b))) + 4, 18)
    return (1 ^ (2 if (4 if 8 < 3 else 8) < total(build(2, 18)) else pair(a, b)[1]))


def main():
    y = 10
    f = lambda x: x * (13 if ap(lambda x: x * 17 + 3, 10) < total(build(1, y)) else (y if y < 5 else y)) + y
    l = build(6, y)
    return (ap(lambda x: x * (total(build(1, 15)) if (14 ^ 2) < pair(3, 8)[1] else pair(0, 2)[0]) + 1, ap(lambda x: x * pair(19, 1)[0] + 0, ap(lambda x: x * 12 + 4, 7))), ap(lambda x: x * total(build(4, h2(y, y))) + 7, h0(pair(13, y)[0], ap(lambda x: x * 15 + 8, y))), f(ap(lambda x: x * (total(build(1, 15)) if (14 ^ 2) < pair(3, 8)[1] else pair(0, 2)[0]) + 1, ap(lambda x: x * pair(19, 1)[0] + 0, ap(lambda x: x * 12 + 4, 7)))) + f(y), total(l) + total(l))
