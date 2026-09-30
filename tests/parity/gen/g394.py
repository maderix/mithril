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
    x = (b if total(build(3, pair(b, a)[1])) < pair(pair(b, a)[1], ap(lambda x: x * 14 + 3, b))[0] else a)
    return (total(build(5, x)) if 10 < total(build(6, total(build(3, 3)))) else 18)


def h1(a, b):
    x = (2 if pair((8 if 12 < 0 else b), h0(0, 17))[1] < (ap(lambda x: x * 17 + 2, 16) ^ a) else total(build(3, 11)))
    return 19


def h2(a, b):
    x = total(build(4, (ap(lambda x: x * 10 + 0, 8) * (1 if 17 < 15 else 5))))
    return ap(lambda x: x * 16 + 3, (pair(x, b)[0] * total(build(5, b))))


def main():
    y = h1(((17 + 14) if total(build(5, 0)) < total(build(0, 8)) else ap(lambda x: x * 7 + 7, 5)), h1(pair(15, 2)[0], h2(19, 7)))
    f = lambda x: x * ap(lambda x: x * total(build(3, y)) + 7, 16) + y
    l = build(3, y)
    return (pair(4, total(build(5, (y ^ 0))))[1], (9 if total(build(0, (3 if 8 < 10 else y))) < y else total(build(4, ap(lambda x: x * 5 + 0, 12)))), f(pair(4, total(build(5, (y ^ 0))))[1]) + f(y), total(l) + total(l))
