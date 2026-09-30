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
    x = total(build(1, ((16 if 3 < 13 else 1) if a < a else pair(0, b)[1])))
    return (ap(lambda x: x * total(build(1, a)) + 0, ap(lambda x: x * 12 + 4, 10)) + 5)


def h1(a, b):
    x = (ap(lambda x: x * h0(a, 1) + 6, total(build(4, 16))) if pair(h0(4, a), 9)[0] < h0(1, (10 * b)) else pair(a, ap(lambda x: x * a + 3, 8))[0])
    return 8


def h2(a, b):
    x = total(build(2, pair(h0(0, 1), 9)[1]))
    return total(build(1, x))


def main():
    y = h0(h0((3 + 1), pair(13, 4)[1]), total(build(3, (11 if 2 < 4 else 2))))
    f = lambda x: x * 16 + y
    l = build(3, y)
    return ((((y ^ 8) ^ total(build(5, 17))) if (h2(y, y) if (19 if 3 < 2 else y) < total(build(0, y)) else (13 & 8)) < pair(h2(y, 17), ap(lambda x: x * y + 4, 16))[0] else ap(lambda x: x * total(build(4, y)) + 3, (y if 0 < y else 3))), y, f((((y ^ 8) ^ total(build(5, 17))) if (h2(y, y) if (19 if 3 < 2 else y) < total(build(0, y)) else (13 & 8)) < pair(h2(y, 17), ap(lambda x: x * y + 4, 16))[0] else ap(lambda x: x * total(build(4, y)) + 3, (y if 0 < y else 3)))) + f(y), total(l) + total(l))
