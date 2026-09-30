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
    x = pair((b if total(build(4, 1)) < total(build(1, 17)) else a), 6)[1]
    return pair(((b - b) if total(build(2, 5)) < b else 1), (a + ap(lambda x: x * 11 + 0, b)))[1]


def h1(a, b):
    x = ap(lambda x: x * total(build(0, a)) + 1, total(build(5, b)))
    return 3


def h2(a, b):
    x = ap(lambda x: x * h0((0 - 15), (a & b)) + 3, (h1(18, b) if a < (14 - b) else h0(11, b)))
    return total(build(6, pair((x ^ b), total(build(2, x)))[1]))


def main():
    y = ((15 - (16 - 0)) ^ total(build(5, (2 if 3 < 18 else 18))))
    f = lambda x: x * ((17 if 8 < y else 10) * total(build(2, y))) + y
    l = build(2, y)
    return (pair(y, (ap(lambda x: x * y + 6, y) if (y * 1) < total(build(4, y)) else pair(y, 14)[1]))[1], h1((14 if 18 < y else pair(y, 15)[0]), y), f(pair(y, (ap(lambda x: x * y + 6, y) if (y * 1) < total(build(4, y)) else pair(y, 14)[1]))[1]) + f(y), total(l) + total(l))
