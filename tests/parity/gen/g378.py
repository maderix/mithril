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
    x = ap(lambda x: x * total(build(5, 19)) + 2, a)
    return (13 - ap(lambda x: x * 14 + 2, total(build(2, x))))


def h1(a, b):
    x = a
    return total(build(3, pair(h0(11, 18), x)[0]))


def h2(a, b):
    x = ap(lambda x: x * ((10 - a) + b) + 3, h0(h0(9, b), a))
    return a


def main():
    y = h0(((0 ^ 5) if 14 < pair(9, 11)[1] else (7 & 8)), ap(lambda x: x * h2(10, 1) + 1, total(build(2, 12))))
    f = lambda x: x * (pair(6, 10)[0] if (y if 7 < 18 else y) < pair(13, y)[1] else pair(12, 12)[1]) + y
    l = build(5, y)
    return (y, h1((total(build(6, 12)) if total(build(4, y)) < total(build(1, y)) else pair(y, 0)[0]), total(build(3, (4 - y)))), f(y) + f(y), total(l) + total(l))
