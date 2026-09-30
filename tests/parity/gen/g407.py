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
    x = b
    return pair((15 - a), 5)[1]


def h1(a, b):
    x = ((total(build(3, a)) - h0(a, 16)) if total(build(4, ap(lambda x: x * a + 8, 19))) < b else pair(total(build(1, 13)), b)[1])
    return ((ap(lambda x: x * a + 1, 15) if (x + x) < h0(10, x) else total(build(6, 8))) if h0(a, (11 * a)) < total(build(3, ap(lambda x: x * 3 + 6, b))) else pair(16, (10 if x < 2 else b))[1])


def h2(a, b):
    x = a
    return pair(4, (a & (4 * 14)))[0]


def main():
    y = (total(build(5, 10)) if h2(7, h0(18, 11)) < 6 else total(build(5, (13 - 12))))
    f = lambda x: x * 10 + y
    l = build(4, y)
    return (19, ((6 - total(build(1, y))) if pair(y, pair(19, y)[0])[0] < 14 else (total(build(6, 3)) if pair(y, y)[1] < (y if 16 < y else y) else total(build(6, y)))), f(19) + f(y), total(l) + total(l))
