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
    x = 12
    return (a if ap(lambda x: x * ap(lambda x: x * b + 2, a) + 2, a) < x else pair((x - 4), x)[1])


def h1(a, b):
    x = a
    return ap(lambda x: x * (total(build(4, 2)) & ap(lambda x: x * 17 + 4, 1)) + 0, x)


def h2(a, b):
    x = ((a ^ total(build(0, 8))) if pair(h1(b, 19), (12 + 10))[1] < (b & (11 if b < 13 else 2)) else (2 * h1(b, b)))
    return h0(x, 0)


def main():
    y = 14
    f = lambda x: x * pair(18, ap(lambda x: x * y + 0, y))[0] + y
    l = build(4, y)
    return (ap(lambda x: x * h1(ap(lambda x: x * y + 5, 8), (y if 0 < y else y)) + 7, (total(build(5, 16)) if pair(0, y)[1] < pair(y, y)[0] else ap(lambda x: x * y + 1, 19))), total(build(1, ap(lambda x: x * (15 - 11) + 4, y))), f(ap(lambda x: x * h1(ap(lambda x: x * y + 5, 8), (y if 0 < y else y)) + 7, (total(build(5, 16)) if pair(0, y)[1] < pair(y, y)[0] else ap(lambda x: x * y + 1, 19)))) + f(y), total(l) + total(l))
