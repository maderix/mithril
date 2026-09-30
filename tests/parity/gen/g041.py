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
    x = (7 if (pair(a, b)[1] - total(build(3, 19))) < pair(pair(9, a)[0], (b ^ 14))[0] else 0)
    return 0


def h1(a, b):
    x = h0(total(build(3, ap(lambda x: x * 0 + 6, a))), (ap(lambda x: x * a + 7, a) if a < pair(b, b)[1] else pair(a, 13)[1]))
    return 3


def h2(a, b):
    x = (12 if ap(lambda x: x * pair(a, 15)[1] + 4, total(build(1, 0))) < pair(pair(2, 14)[0], a)[0] else pair(total(build(0, 17)), b)[1])
    return pair(b, h0(pair(14, a)[0], (x if b < x else 5)))[0]


def main():
    y = pair(pair(h0(4, 8), 1)[1], 9)[0]
    f = lambda x: x * (ap(lambda x: x * 10 + 6, 16) if (y * 5) < pair(7, 7)[0] else ap(lambda x: x * y + 1, 4)) + y
    l = build(1, y)
    return (pair(ap(lambda x: x * (2 & y) + 5, total(build(4, y))), (ap(lambda x: x * y + 2, 18) * total(build(4, y))))[1], total(build(4, total(build(2, (17 ^ 9))))), f(pair(ap(lambda x: x * (2 & y) + 5, total(build(4, y))), (ap(lambda x: x * y + 2, 18) * total(build(4, y))))[1]) + f(y), total(l) + total(l))
