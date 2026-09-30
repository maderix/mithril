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
    x = (total(build(5, (15 & b))) + 14)
    return pair(ap(lambda x: x * total(build(2, 10)) + 3, 7), (17 if ap(lambda x: x * x + 4, b) < pair(14, a)[1] else b))[0]


def h1(a, b):
    x = a
    return 11


def h2(a, b):
    x = pair(a, ap(lambda x: x * ap(lambda x: x * b + 1, 7) + 0, ap(lambda x: x * b + 2, 17)))[0]
    return pair(a, (x if total(build(3, 9)) < ap(lambda x: x * 18 + 4, a) else (19 if b < 19 else 6)))[0]


def main():
    y = h1(ap(lambda x: x * ap(lambda x: x * 19 + 5, 7) + 2, (14 if 8 < 12 else 13)), total(build(3, 13)))
    f = lambda x: x * h0(y, ap(lambda x: x * 11 + 5, y)) + y
    l = build(1, y)
    return (10, pair(ap(lambda x: x * y + 6, pair(y, y)[1]), total(build(3, (y ^ y))))[1], f(10) + f(y), total(l) + total(l))
