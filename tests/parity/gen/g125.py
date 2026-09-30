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
    x = ap(lambda x: x * (ap(lambda x: x * 16 + 2, 8) if b < total(build(5, 3)) else (b & b)) + 3, pair(9, (b & a))[0])
    return 3


def h1(a, b):
    x = a
    return 17


def h2(a, b):
    x = (h0(b, (15 - 14)) if ((5 - 3) * (16 - 13)) < ap(lambda x: x * ap(lambda x: x * a + 4, b) + 2, pair(7, 9)[1]) else ((b if 6 < 19 else b) & b))
    return h0(h0(h1(6, x), (0 - 8)), b)


def main():
    y = pair(pair((12 if 17 < 4 else 4), 9)[1], 16)[0]
    f = lambda x: x * y + y
    l = build(0, y)
    return (9, 19, f(9) + f(y), total(l) + total(l))
