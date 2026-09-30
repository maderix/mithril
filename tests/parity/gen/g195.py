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
    return pair((6 * pair(a, a)[0]), total(build(3, ap(lambda x: x * 17 + 0, 0))))[1]


def h1(a, b):
    x = (pair(total(build(3, b)), (5 if 7 < 13 else 18))[0] if a < h0(9, pair(a, a)[0]) else h0((a if b < a else a), pair(1, a)[0]))
    return (x if ap(lambda x: x * ap(lambda x: x * 9 + 6, 16) + 0, ap(lambda x: x * x + 5, 17)) < (x if x < (17 if x < a else 7) else ap(lambda x: x * a + 6, x)) else total(build(6, total(build(0, a)))))


def h2(a, b):
    x = b
    return h0(5, x)


def main():
    y = h2((ap(lambda x: x * 3 + 1, 11) if (3 + 16) < ap(lambda x: x * 10 + 4, 16) else total(build(0, 4))), 0)
    f = lambda x: x * total(build(1, (0 * y))) + y
    l = build(4, y)
    return (pair(y, (h2(y, 7) if ap(lambda x: x * y + 3, 3) < h2(16, 3) else 6))[0], pair(y, (pair(14, 7)[0] & (y + 12)))[0], f(pair(y, (h2(y, 7) if ap(lambda x: x * y + 3, 3) < h2(16, 3) else 6))[0]) + f(y), total(l) + total(l))
