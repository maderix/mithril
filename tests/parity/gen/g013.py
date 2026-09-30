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
    x = (0 if (0 + (10 if 13 < 9 else 17)) < 17 else pair((a & a), total(build(3, 1)))[1])
    return 6


def h1(a, b):
    x = h0((13 if (16 + 5) < a else a), ap(lambda x: x * pair(a, 9)[0] + 3, ap(lambda x: x * a + 8, 16)))
    return total(build(4, h0(3, (3 + b))))


def h2(a, b):
    x = h0(ap(lambda x: x * pair(4, b)[0] + 8, 19), 4)
    return pair(4, total(build(0, pair(a, 6)[1])))[1]


def main():
    y = 2
    f = lambda x: x * pair(pair(y, 8)[1], ap(lambda x: x * y + 5, 13))[0] + y
    l = build(4, y)
    return (h2(((6 if 14 < y else y) if ap(lambda x: x * 10 + 6, 10) < (15 & y) else ap(lambda x: x * 12 + 0, 4)), (y ^ (y if 18 < 13 else 15))), pair(ap(lambda x: x * total(build(3, y)) + 5, total(build(4, 17))), ap(lambda x: x * ap(lambda x: x * 18 + 7, 6) + 7, total(build(6, y))))[0], f(h2(((6 if 14 < y else y) if ap(lambda x: x * 10 + 6, 10) < (15 & y) else ap(lambda x: x * 12 + 0, 4)), (y ^ (y if 18 < 13 else 15)))) + f(y), total(l) + total(l))
