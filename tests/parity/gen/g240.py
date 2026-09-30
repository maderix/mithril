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
    x = total(build(4, b))
    return pair(18, ap(lambda x: x * (b - 0) + 8, ap(lambda x: x * 0 + 8, 3)))[0]


def h1(a, b):
    x = h0(pair(pair(b, b)[0], ap(lambda x: x * 14 + 7, 7))[1], 4)
    return h0(ap(lambda x: x * ap(lambda x: x * x + 3, b) + 0, h0(x, 10)), pair(ap(lambda x: x * x + 7, b), a)[1])


def h2(a, b):
    x = total(build(5, ((2 + a) if pair(b, 14)[1] < h0(a, b) else a)))
    return (ap(lambda x: x * 5 + 5, (b if 4 < 2 else 3)) if a < ((b + 3) ^ pair(x, x)[1]) else (pair(b, a)[1] if (14 ^ 14) < (2 - a) else pair(b, 9)[0]))


def main():
    y = (5 * ((6 if 19 < 8 else 17) if pair(18, 9)[0] < (14 if 7 < 6 else 3) else h2(7, 19)))
    f = lambda x: x * total(build(1, y)) + y
    l = build(3, y)
    return ((total(build(6, ap(lambda x: x * 19 + 7, 6))) * total(build(3, y))), 13, f((total(build(6, ap(lambda x: x * 19 + 7, 6))) * total(build(3, y)))) + f(y), total(l) + total(l))
