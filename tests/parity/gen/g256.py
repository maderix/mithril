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
    x = (a if (8 if ap(lambda x: x * a + 6, 6) < (a + b) else ap(lambda x: x * b + 7, a)) < 13 else ap(lambda x: x * pair(a, b)[0] + 8, 13))
    return (9 - x)


def h1(a, b):
    x = h0(total(build(0, h0(19, 9))), total(build(2, ap(lambda x: x * a + 5, b))))
    return h0(x, h0(ap(lambda x: x * 5 + 2, 18), pair(14, 12)[1]))


def h2(a, b):
    x = h0(0, ap(lambda x: x * total(build(4, 18)) + 6, b))
    return 15


def main():
    y = (pair(pair(1, 7)[1], ap(lambda x: x * 16 + 5, 7))[1] & h2(pair(9, 4)[1], (5 + 15)))
    f = lambda x: x * y + y
    l = build(1, y)
    return (pair(ap(lambda x: x * (y if 9 < 8 else y) + 8, h1(4, 7)), total(build(5, total(build(1, y)))))[0], y, f(pair(ap(lambda x: x * (y if 9 < 8 else y) + 8, h1(4, 7)), total(build(5, total(build(1, y)))))[0]) + f(y), total(l) + total(l))
