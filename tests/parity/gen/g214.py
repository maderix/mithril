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
    x = 2
    return 13


def h1(a, b):
    x = h0(a, (h0(b, 16) if total(build(4, 19)) < (b + 2) else (b - 18)))
    return ap(lambda x: x * total(build(5, h0(x, 7))) + 4, b)


def h2(a, b):
    x = ap(lambda x: x * 12 + 7, b)
    return pair(pair(b, total(build(4, a)))[0], total(build(5, (8 + a))))[1]


def main():
    y = ap(lambda x: x * total(build(3, pair(7, 0)[0])) + 0, (ap(lambda x: x * 10 + 4, 3) + (7 - 19)))
    f = lambda x: x * (pair(0, y)[1] * ap(lambda x: x * y + 3, 3)) + y
    l = build(6, y)
    return ((1 - total(build(5, pair(y, 15)[1]))), (h0((7 - y), 9) if pair(y, (5 & y))[0] < h2((1 if y < y else y), 19) else pair(y, 16)[0]), f((1 - total(build(5, pair(y, 15)[1])))) + f(y), total(l) + total(l))
