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
    x = 8
    return ap(lambda x: x * (9 if b < total(build(3, x)) else ap(lambda x: x * 1 + 3, 10)) + 7, a)


def h1(a, b):
    x = h0(total(build(6, (a + a))), b)
    return total(build(0, b))


def h2(a, b):
    x = ((h0(a, 4) & (8 if 5 < 18 else 7)) if (a - pair(a, 15)[0]) < total(build(0, h1(b, b))) else h0((17 if 12 < 8 else b), a))
    return h0(ap(lambda x: x * 4 + 7, (7 & x)), h0(7, 0))


def main():
    y = ap(lambda x: x * (2 if ap(lambda x: x * 2 + 2, 18) < (6 if 17 < 12 else 9) else 0) + 4, (total(build(1, 11)) * (19 ^ 15)))
    f = lambda x: x * (y if h1(y, y) < total(build(1, 14)) else (6 if y < 14 else 12)) + y
    l = build(4, y)
    return (pair(pair((y if y < y else y), y)[1], ((10 if y < 3 else y) ^ pair(y, y)[1]))[1], y, f(pair(pair((y if y < y else y), y)[1], ((10 if y < 3 else y) ^ pair(y, y)[1]))[1]) + f(y), total(l) + total(l))
