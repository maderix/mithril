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
    x = ap(lambda x: x * ap(lambda x: x * (5 if b < 15 else b) + 6, 19) + 1, (ap(lambda x: x * b + 5, 7) if 12 < pair(a, a)[1] else a))
    return total(build(5, ap(lambda x: x * ap(lambda x: x * 0 + 6, x) + 6, 5)))


def h1(a, b):
    x = ap(lambda x: x * h0(15, pair(14, b)[0]) + 6, (pair(11, 16)[0] - pair(19, 2)[1]))
    return (16 if (h0(14, a) if 18 < 10 else h0(a, 3)) < total(build(2, total(build(2, 10)))) else total(build(6, total(build(3, 5)))))


def h2(a, b):
    x = (pair(h1(16, a), h1(5, a))[1] if pair(total(build(4, a)), h1(a, 1))[0] < b else pair((b if a < 1 else 12), (17 ^ 12))[1])
    return (pair(total(build(2, 18)), (17 if 10 < 14 else 9))[1] if ap(lambda x: x * total(build(2, a)) + 8, (x if 7 < 18 else 10)) < (h1(b, 4) - (x & a)) else a)


def main():
    y = 10
    f = lambda x: x * (h2(4, 2) ^ y) + y
    l = build(4, y)
    return (ap(lambda x: x * ap(lambda x: x * y + 8, y) + 0, ap(lambda x: x * y + 6, (y if 9 < 18 else 10))), total(build(2, pair(ap(lambda x: x * y + 2, 12), y)[0])), f(ap(lambda x: x * ap(lambda x: x * y + 8, y) + 0, ap(lambda x: x * y + 6, (y if 9 < 18 else 10)))) + f(y), total(l) + total(l))
