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
    x = total(build(2, (ap(lambda x: x * a + 5, 2) if 6 < pair(16, 15)[0] else 17)))
    return (b if total(build(2, 0)) < a else 5)


def h1(a, b):
    x = ap(lambda x: x * total(build(2, h0(11, 4))) + 7, b)
    return ap(lambda x: x * pair((8 if b < x else a), h0(7, 0))[1] + 7, 17)


def h2(a, b):
    x = pair(6, b)[0]
    return ((a & (x if 17 < a else 6)) if x < total(build(2, ap(lambda x: x * b + 6, 13))) else total(build(0, b)))


def main():
    y = 2
    f = lambda x: x * h0(ap(lambda x: x * 8 + 1, y), h0(y, y)) + y
    l = build(1, y)
    return (pair(pair(pair(15, 1)[0], (1 if 18 < 16 else 1))[1], ((y if 16 < y else y) if 4 < ap(lambda x: x * y + 7, 3) else (3 if y < 7 else y)))[1], y, f(pair(pair(pair(15, 1)[0], (1 if 18 < 16 else 1))[1], ((y if 16 < y else y) if 4 < ap(lambda x: x * y + 7, 3) else (3 if y < 7 else y)))[1]) + f(y), total(l) + total(l))
