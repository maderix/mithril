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
    x = 15
    return total(build(1, (18 & pair(a, 7)[1])))


def h1(a, b):
    x = 17
    return (b ^ ap(lambda x: x * b + 8, total(build(6, 9))))


def h2(a, b):
    x = ap(lambda x: x * h0(h0(b, 5), (11 - b)) + 3, (14 * (12 + 18)))
    return (((12 * x) if (b + a) < total(build(0, 4)) else pair(9, 16)[0]) if (ap(lambda x: x * 15 + 2, a) if b < pair(b, a)[1] else total(build(5, b))) < (h0(x, 10) if (1 if b < 11 else b) < b else b) else h1((x if b < 11 else 3), total(build(0, x))))


def main():
    y = pair(15, h1((12 + 4), 14))[0]
    f = lambda x: x * ap(lambda x: x * pair(y, 3)[1] + 3, (10 ^ y)) + y
    l = build(5, y)
    return (pair(h2(y, ap(lambda x: x * 12 + 8, 1)), 17)[1], ap(lambda x: x * y + 7, (h2(1, 2) if h2(12, y) < (9 if 1 < y else 18) else y)), f(pair(h2(y, ap(lambda x: x * 12 + 8, 1)), 17)[1]) + f(y), total(l) + total(l))
