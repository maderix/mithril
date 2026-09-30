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
    x = 14
    return 7


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * pair(15, a)[1] + 5, h0(a, a)) + 5, total(build(5, b)))
    return (pair(pair(2, b)[0], ap(lambda x: x * 19 + 4, 3))[1] if a < pair(8, (3 if x < 16 else b))[1] else total(build(5, (2 if 11 < x else 1))))


def h2(a, b):
    x = a
    return h0(pair(h1(b, 18), total(build(4, 0)))[1], (1 if b < total(build(2, a)) else (18 if 16 < 14 else b)))


def main():
    y = h1(0, pair(ap(lambda x: x * 19 + 3, 6), (13 - 11))[1])
    f = lambda x: x * total(build(3, y)) + y
    l = build(1, y)
    return (h2(ap(lambda x: x * ap(lambda x: x * y + 7, y) + 8, y), ap(lambda x: x * total(build(6, y)) + 2, (y & 17))), ap(lambda x: x * ((y - y) if h2(8, 18) < total(build(5, 8)) else ap(lambda x: x * y + 1, y)) + 4, (8 & h0(3, 4))), f(h2(ap(lambda x: x * ap(lambda x: x * y + 7, y) + 8, y), ap(lambda x: x * total(build(6, y)) + 2, (y & 17)))) + f(y), total(l) + total(l))
