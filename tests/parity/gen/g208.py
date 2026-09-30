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
    x = ((a - 5) if 9 < ap(lambda x: x * pair(a, b)[1] + 1, total(build(6, a))) else total(build(4, ap(lambda x: x * 7 + 6, a))))
    return (a if ap(lambda x: x * total(build(4, 2)) + 6, x) < total(build(6, 12)) else b)


def h1(a, b):
    x = h0(a, b)
    return h0(pair(total(build(6, x)), (3 * 15))[0], 15)


def h2(a, b):
    x = h0(14, (ap(lambda x: x * a + 4, b) & (17 if a < 15 else a)))
    return a


def main():
    y = ap(lambda x: x * h1(ap(lambda x: x * 17 + 8, 5), 7) + 6, (ap(lambda x: x * 3 + 8, 7) + (8 ^ 8)))
    f = lambda x: x * y + y
    l = build(5, y)
    return ((y if (ap(lambda x: x * 18 + 3, 18) - total(build(1, y))) < total(build(0, pair(19, 9)[0])) else 19), ap(lambda x: x * y + 1, 16), f((y if (ap(lambda x: x * 18 + 3, 18) - total(build(1, y))) < total(build(0, pair(19, 9)[0])) else 19)) + f(y), total(l) + total(l))
