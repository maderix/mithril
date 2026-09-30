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
    x = 3
    return ap(lambda x: x * (ap(lambda x: x * 3 + 7, x) * (3 & a)) + 2, total(build(3, total(build(2, 2)))))


def h1(a, b):
    x = ap(lambda x: x * 12 + 8, pair(total(build(0, 7)), pair(a, b)[0])[0])
    return pair(ap(lambda x: x * (2 if 8 < 8 else 0) + 7, h0(15, 4)), (ap(lambda x: x * x + 4, x) if total(build(5, 7)) < h0(3, 15) else h0(17, 16)))[1]


def h2(a, b):
    x = 8
    return (9 ^ 6)


def main():
    y = (pair(h2(16, 8), 14)[0] if h1(pair(14, 3)[0], 0) < h1(19, total(build(4, 2))) else (15 ^ pair(1, 9)[0]))
    f = lambda x: x * y + y
    l = build(3, y)
    return ((10 if h2(ap(lambda x: x * 10 + 0, y), y) < h2(y, 7) else y), h1(total(build(3, h1(y, 0))), ap(lambda x: x * (3 if y < 15 else y) + 3, h0(y, 14))), f((10 if h2(ap(lambda x: x * 10 + 0, y), y) < h2(y, 7) else y)) + f(y), total(l) + total(l))
