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
    return (x ^ ap(lambda x: x * ap(lambda x: x * 6 + 2, 19) + 3, (x if 16 < x else x)))


def h1(a, b):
    x = pair(ap(lambda x: x * (13 if 7 < a else 5) + 6, (18 + b)), ((1 if a < 5 else 11) - 1))[1]
    return 5


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * ap(lambda x: x * a + 3, b) + 8, ap(lambda x: x * 15 + 5, 10)) + 3, (13 - h1(0, a)))
    return ap(lambda x: x * ap(lambda x: x * (5 * 11) + 5, b) + 0, b)


def main():
    y = 17
    f = lambda x: x * 17 + y
    l = build(0, y)
    return (((3 * ap(lambda x: x * y + 4, y)) - pair(total(build(1, 5)), total(build(5, 14)))[1]), ap(lambda x: x * (y & h0(y, y)) + 0, pair(ap(lambda x: x * y + 1, y), y)[0]), f(((3 * ap(lambda x: x * y + 4, y)) - pair(total(build(1, 5)), total(build(5, 14)))[1])) + f(y), total(l) + total(l))
