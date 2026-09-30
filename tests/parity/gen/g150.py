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
    x = pair((b ^ 11), total(build(3, ap(lambda x: x * b + 0, b))))[1]
    return a


def h1(a, b):
    x = pair(pair(pair(13, b)[1], (13 + a))[1], h0(ap(lambda x: x * b + 2, 4), 9))[1]
    return ap(lambda x: x * (h0(7, 8) if pair(9, a)[1] < h0(19, b) else total(build(1, x))) + 8, ((a ^ 15) - ap(lambda x: x * 19 + 2, x)))


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * (a & a) + 8, a) + 5, (total(build(2, b)) if 3 < ap(lambda x: x * b + 0, a) else pair(11, b)[1]))
    return (h1(h0(a, a), 19) ^ (x - (6 & x)))


def main():
    y = total(build(1, (13 if 14 < 15 else 17)))
    f = lambda x: x * h2(total(build(3, y)), (0 - 11)) + y
    l = build(5, y)
    return (pair(h1(total(build(3, 14)), 2), y)[0], (((14 if y < 19 else 16) ^ h1(11, 17)) if total(build(3, y)) < h2(y, h1(12, y)) else total(build(2, y))), f(pair(h1(total(build(3, 14)), 2), y)[0]) + f(y), total(l) + total(l))
