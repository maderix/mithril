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
    x = (2 if 14 < 12 else (pair(b, 6)[0] if 11 < pair(b, 2)[0] else total(build(3, b))))
    return ap(lambda x: x * x + 5, (pair(9, x)[1] & total(build(2, b))))


def h1(a, b):
    x = total(build(1, (ap(lambda x: x * a + 8, 13) if total(build(2, a)) < total(build(6, a)) else b)))
    return total(build(6, h0(13, ap(lambda x: x * 16 + 8, a))))


def h2(a, b):
    x = pair(ap(lambda x: x * ap(lambda x: x * b + 2, 10) + 2, ap(lambda x: x * a + 6, b)), total(build(3, total(build(3, 10)))))[0]
    return (h0(h0(x, a), ap(lambda x: x * 10 + 5, 6)) * ((3 + b) ^ (18 & x)))


def main():
    y = total(build(3, 10))
    f = lambda x: x * ((15 ^ y) if pair(y, y)[1] < ap(lambda x: x * y + 6, 9) else (14 * y)) + y
    l = build(4, y)
    return (total(build(4, (total(build(6, 5)) + y))), (h2((8 + 2), y) if (ap(lambda x: x * y + 8, 1) - ap(lambda x: x * y + 8, 16)) < total(build(3, h0(8, y))) else y), f(total(build(4, (total(build(6, 5)) + y)))) + f(y), total(l) + total(l))
