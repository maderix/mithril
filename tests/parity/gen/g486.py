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
    x = 6
    return ap(lambda x: x * (9 & (13 if x < 0 else 7)) + 0, x)


def h1(a, b):
    x = b
    return h0(((10 - 7) if a < (19 if a < 17 else a) else 6), ap(lambda x: x * (a if 18 < x else 17) + 5, ap(lambda x: x * 7 + 4, 11)))


def h2(a, b):
    x = h1(a, ((12 if 11 < 7 else a) * (b - a)))
    return total(build(0, ((15 if b < a else 11) - pair(a, b)[0])))


def main():
    y = (18 if 17 < (total(build(3, 0)) - h0(9, 10)) else ap(lambda x: x * (5 if 13 < 18 else 1) + 4, (9 if 6 < 13 else 8)))
    f = lambda x: x * ((5 if y < y else y) if total(build(3, 7)) < ap(lambda x: x * 2 + 4, 0) else y) + y
    l = build(1, y)
    return (total(build(4, ap(lambda x: x * ap(lambda x: x * y + 7, 2) + 1, y))), h0(y, pair(h1(17, 16), y)[0]), f(total(build(4, ap(lambda x: x * ap(lambda x: x * y + 7, 2) + 1, y)))) + f(y), total(l) + total(l))
