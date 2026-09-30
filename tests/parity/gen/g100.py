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
    x = ap(lambda x: x * total(build(4, total(build(3, 2)))) + 7, (10 & total(build(1, a))))
    return (6 if total(build(2, 3)) < x else (total(build(4, 7)) & total(build(5, 11))))


def h1(a, b):
    x = pair((a + total(build(2, 17))), total(build(2, (b if 10 < 4 else 17))))[0]
    return total(build(4, ap(lambda x: x * h0(17, a) + 4, a)))


def h2(a, b):
    x = ap(lambda x: x * ((a * b) if (a + a) < pair(15, 12)[1] else (2 if b < 15 else 11)) + 8, h1((14 + a), total(build(2, b))))
    return 17


def main():
    y = (5 if ap(lambda x: x * (3 if 11 < 5 else 9) + 6, ap(lambda x: x * 8 + 7, 6)) < h0(total(build(5, 5)), (5 & 6)) else pair(h1(18, 18), 17)[0])
    f = lambda x: x * ((2 - 8) if h2(y, 13) < 2 else y) + y
    l = build(3, y)
    return (h2(pair(h2(y, y), (y if 5 < y else 13))[1], ap(lambda x: x * ap(lambda x: x * y + 0, y) + 0, 13)), (h0((y if 15 < y else 19), total(build(1, 4))) if ap(lambda x: x * y + 5, (3 - 1)) < total(build(6, h1(y, y))) else (total(build(1, 13)) + y)), f(h2(pair(h2(y, y), (y if 5 < y else 13))[1], ap(lambda x: x * ap(lambda x: x * y + 0, y) + 0, 13))) + f(y), total(l) + total(l))
