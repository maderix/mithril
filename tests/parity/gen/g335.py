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
    x = ap(lambda x: x * pair(pair(2, 11)[1], (16 & a))[1] + 4, 5)
    return 9


def h1(a, b):
    x = a
    return ap(lambda x: x * (x & x) + 5, (pair(6, 17)[0] if h0(12, 11) < (11 if 9 < 15 else a) else 1))


def h2(a, b):
    x = a
    return b


def main():
    y = pair(((6 - 11) - 16), 5)[1]
    f = lambda x: x * (total(build(1, y)) if (2 + y) < pair(16, 16)[0] else h2(y, y)) + y
    l = build(6, y)
    return ((total(build(6, (y ^ y))) if (14 if h0(15, 18) < pair(y, y)[1] else (0 * 11)) < pair((y if 4 < 5 else y), ap(lambda x: x * 0 + 8, 1))[1] else (total(build(1, y)) if ap(lambda x: x * 8 + 5, 12) < y else total(build(0, 11)))), y, f((total(build(6, (y ^ y))) if (14 if h0(15, 18) < pair(y, y)[1] else (0 * 11)) < pair((y if 4 < 5 else y), ap(lambda x: x * 0 + 8, 1))[1] else (total(build(1, y)) if ap(lambda x: x * 8 + 5, 12) < y else total(build(0, 11))))) + f(y), total(l) + total(l))
