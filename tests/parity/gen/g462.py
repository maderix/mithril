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
    x = 7
    return 8


def h1(a, b):
    x = pair(pair(0, (a if a < 14 else b))[1], ap(lambda x: x * pair(b, 0)[0] + 3, 18))[0]
    return total(build(3, h0(pair(9, a)[1], x)))


def h2(a, b):
    x = (((14 if a < b else 11) if h0(b, a) < (b & a) else total(build(4, 3))) if (19 + (a * b)) < pair(14, b)[1] else (ap(lambda x: x * 4 + 7, 3) if 6 < pair(a, b)[0] else pair(6, 3)[0]))
    return (4 + h1(16, pair(x, x)[0]))


def main():
    y = 18
    f = lambda x: x * (total(build(6, y)) if ap(lambda x: x * y + 0, y) < pair(y, 6)[0] else 5) + y
    l = build(3, y)
    return (((11 if ap(lambda x: x * 12 + 4, y) < total(build(5, 0)) else 9) + 10), pair(total(build(6, (y - y))), ap(lambda x: x * h0(13, 16) + 4, h0(y, y)))[1], f(((11 if ap(lambda x: x * 12 + 4, y) < total(build(5, 0)) else 9) + 10)) + f(y), total(l) + total(l))
