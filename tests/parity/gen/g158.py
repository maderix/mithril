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
    x = total(build(6, (pair(a, b)[0] if 7 < 8 else (19 & 12))))
    return pair(14, (a if total(build(2, b)) < total(build(1, b)) else ap(lambda x: x * a + 6, a)))[0]


def h1(a, b):
    x = pair(b, b)[0]
    return pair(((a & 3) - (16 * 1)), ap(lambda x: x * 7 + 1, h0(b, x)))[0]


def h2(a, b):
    x = (14 if ap(lambda x: x * total(build(0, 13)) + 2, (18 if b < b else 17)) < total(build(4, (12 if 19 < b else a))) else pair(ap(lambda x: x * 3 + 7, 9), (5 if b < 2 else 5))[0])
    return (ap(lambda x: x * ap(lambda x: x * 11 + 5, 14) + 0, a) * ((a if a < 3 else a) if 12 < total(build(1, 6)) else (8 if a < 2 else 14)))


def main():
    y = ap(lambda x: x * ap(lambda x: x * ap(lambda x: x * 0 + 3, 16) + 2, 16) + 1, (16 if (18 if 14 < 4 else 10) < ap(lambda x: x * 8 + 3, 6) else h0(0, 5)))
    f = lambda x: x * h0((1 if 13 < 8 else y), h1(y, y)) + y
    l = build(0, y)
    return (total(build(3, total(build(2, (9 - y))))), total(build(2, 2)), f(total(build(3, total(build(2, (9 - y)))))) + f(y), total(l) + total(l))
