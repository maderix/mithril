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
    x = ap(lambda x: x * (ap(lambda x: x * 0 + 0, a) - pair(3, 13)[1]) + 3, 15)
    return pair(b, ((a ^ 10) + x))[1]


def h1(a, b):
    x = a
    return pair(ap(lambda x: x * ap(lambda x: x * 19 + 6, x) + 7, a), ap(lambda x: x * 11 + 0, x))[0]


def h2(a, b):
    x = (total(build(4, ap(lambda x: x * a + 0, 16))) + pair(b, pair(b, 16)[0])[1])
    return ((17 ^ a) if (ap(lambda x: x * x + 8, 4) & 8) < total(build(2, h1(b, a))) else pair(7, pair(9, 13)[0])[1])


def main():
    y = pair((h1(2, 17) if pair(9, 15)[1] < pair(3, 13)[0] else h0(10, 16)), ((16 * 16) + pair(14, 2)[1]))[1]
    f = lambda x: x * ap(lambda x: x * h0(8, y) + 2, (15 if 7 < y else 15)) + y
    l = build(2, y)
    return (17, h1(9, (y if 1 < ap(lambda x: x * y + 4, y) else total(build(1, 5)))), f(17) + f(y), total(l) + total(l))
