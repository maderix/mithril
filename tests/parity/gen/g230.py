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
    x = (14 - total(build(1, total(build(6, 13)))))
    return pair(6, ((10 ^ 14) ^ (0 & 19)))[0]


def h1(a, b):
    x = ap(lambda x: x * 4 + 7, ap(lambda x: x * ap(lambda x: x * 7 + 5, a) + 6, total(build(4, a))))
    return (1 if 2 < 4 else h0(ap(lambda x: x * 0 + 4, x), pair(0, b)[0]))


def h2(a, b):
    x = 18
    return b


def main():
    y = h2((4 + (10 & 7)), total(build(5, (13 if 6 < 12 else 10))))
    f = lambda x: x * h0(y, ap(lambda x: x * 8 + 0, y)) + y
    l = build(2, y)
    return (h2(pair(ap(lambda x: x * y + 2, y), h2(y, y))[0], total(build(4, total(build(6, y))))), total(build(1, y)), f(h2(pair(ap(lambda x: x * y + 2, y), h2(y, y))[0], total(build(4, total(build(6, y)))))) + f(y), total(l) + total(l))
