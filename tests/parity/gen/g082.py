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
    x = total(build(2, total(build(0, total(build(5, 4))))))
    return 19


def h1(a, b):
    x = ap(lambda x: x * a + 2, ap(lambda x: x * 18 + 7, (2 & a)))
    return (b if h0(pair(18, x)[0], pair(11, 14)[0]) < h0(14, pair(a, a)[1]) else (4 if h0(b, x) < total(build(6, x)) else 10))


def h2(a, b):
    x = (total(build(2, (a if b < b else a))) if pair((b if 6 < b else a), (3 if 6 < 10 else a))[1] < ap(lambda x: x * h1(1, b) + 8, (a - 14)) else (h1(19, 9) if 12 < h1(11, a) else (b if 11 < 5 else 4)))
    return total(build(2, 11))


def main():
    y = total(build(5, pair(h0(2, 9), 18)[1]))
    f = lambda x: x * pair(pair(4, 17)[0], pair(y, y)[0])[0] + y
    l = build(0, y)
    return (h0(13, ap(lambda x: x * (y ^ 10) + 3, y)), pair((total(build(6, y)) if pair(y, 1)[0] < ap(lambda x: x * 3 + 4, y) else (y if y < 8 else y)), 16)[0], f(h0(13, ap(lambda x: x * (y ^ 10) + 3, y))) + f(y), total(l) + total(l))
