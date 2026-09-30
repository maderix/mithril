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
    x = (5 if ap(lambda x: x * (1 if 11 < 19 else b) + 7, 14) < (17 if ap(lambda x: x * 10 + 2, b) < 19 else (0 ^ b)) else (11 - 14))
    return pair(ap(lambda x: x * 0 + 6, a), (0 & (10 if b < 9 else b)))[1]


def h1(a, b):
    x = a
    return ap(lambda x: x * b + 3, h0(7, pair(14, a)[0]))


def h2(a, b):
    x = pair(total(build(2, ap(lambda x: x * 3 + 6, b))), total(build(2, ap(lambda x: x * b + 5, a))))[1]
    return a


def main():
    y = (h2(8, 0) + (ap(lambda x: x * 12 + 6, 5) + 16))
    f = lambda x: x * y + y
    l = build(6, y)
    return ((16 + (3 if (7 + y) < pair(y, 9)[0] else (0 if y < 12 else y))), (total(build(1, pair(7, y)[0])) - h2(h1(9, y), y)), f((16 + (3 if (7 + y) < pair(y, 9)[0] else (0 if y < 12 else y)))) + f(y), total(l) + total(l))
