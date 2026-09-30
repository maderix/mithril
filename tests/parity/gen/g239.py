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
    x = 1
    return pair(((a if a < 17 else 8) + (10 * x)), x)[0]


def h1(a, b):
    x = ap(lambda x: x * ((a if 2 < a else 10) - (a - a)) + 3, a)
    return (pair(pair(14, 0)[1], (10 - x))[1] if 9 < ap(lambda x: x * h0(8, b) + 2, ap(lambda x: x * b + 1, x)) else 11)


def h2(a, b):
    x = ((a * pair(a, b)[0]) - h1(total(build(0, a)), 9))
    return (ap(lambda x: x * ap(lambda x: x * 9 + 2, a) + 4, ap(lambda x: x * 15 + 2, 13)) if ((b if x < a else x) - total(build(1, 8))) < ((x ^ x) & h1(a, b)) else total(build(4, 5)))


def main():
    y = ((total(build(5, 0)) * pair(4, 16)[1]) ^ total(build(1, h1(7, 10))))
    f = lambda x: x * 14 + y
    l = build(1, y)
    return (ap(lambda x: x * (pair(6, 15)[0] if h1(y, y) < total(build(0, y)) else total(build(3, y))) + 3, 19), 8, f(ap(lambda x: x * (pair(6, 15)[0] if h1(y, y) < total(build(0, y)) else total(build(3, y))) + 3, 19)) + f(y), total(l) + total(l))
