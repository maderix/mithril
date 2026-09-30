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
    x = 0
    return 16


def h1(a, b):
    x = 8
    return 13


def h2(a, b):
    x = 19
    return (h0(h1(a, 16), x) if h1(13, (b * x)) < ((a - 8) - a) else x)


def main():
    y = pair(((8 ^ 11) - h2(5, 9)), (total(build(3, 15)) + (3 if 19 < 7 else 9)))[1]
    f = lambda x: x * pair(15, h2(18, 16))[1] + y
    l = build(0, y)
    return ((h1(y, pair(y, y)[1]) if (total(build(6, 16)) if ap(lambda x: x * y + 1, 12) < 17 else h2(y, y)) < (pair(16, y)[1] if pair(y, 19)[1] < h0(3, 0) else total(build(5, y))) else total(build(4, (19 ^ y)))), (ap(lambda x: x * (y ^ y) + 1, h2(y, 18)) if ((y - 5) if total(build(2, y)) < pair(0, y)[1] else (12 & y)) < h0(pair(10, 2)[0], y) else h0(total(build(0, 8)), total(build(2, y)))), f((h1(y, pair(y, y)[1]) if (total(build(6, 16)) if ap(lambda x: x * y + 1, 12) < 17 else h2(y, y)) < (pair(16, y)[1] if pair(y, 19)[1] < h0(3, 0) else total(build(5, y))) else total(build(4, (19 ^ y))))) + f(y), total(l) + total(l))
