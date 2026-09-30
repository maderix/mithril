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
    x = (14 if ap(lambda x: x * 19 + 1, b) < 18 else ((b & 16) if 18 < 2 else (6 ^ 19)))
    return 13


def h1(a, b):
    x = (total(build(5, ap(lambda x: x * 2 + 4, 14))) if b < ap(lambda x: x * 6 + 4, ap(lambda x: x * 10 + 7, 6)) else ((11 if 12 < 0 else 8) & total(build(4, 6))))
    return a


def h2(a, b):
    x = ap(lambda x: x * (ap(lambda x: x * a + 4, a) ^ pair(2, b)[0]) + 6, pair(h1(a, b), h1(a, 0))[0])
    return total(build(6, a))


def main():
    y = 10
    f = lambda x: x * (ap(lambda x: x * 12 + 5, 3) if 8 < total(build(2, y)) else 9) + y
    l = build(0, y)
    return (y, (total(build(3, total(build(6, y)))) if (y + (4 - y)) < pair(19, pair(16, 14)[1])[1] else 12), f(y) + f(y), total(l) + total(l))
