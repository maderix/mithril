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
    x = 13
    return ap(lambda x: x * x + 6, 2)


def h1(a, b):
    x = h0(pair(h0(a, b), h0(a, 5))[1], 17)
    return ap(lambda x: x * (16 if b < pair(a, x)[0] else pair(11, 4)[1]) + 6, ap(lambda x: x * 15 + 8, (8 if b < x else a)))


def h2(a, b):
    x = total(build(0, 8))
    return total(build(1, x))


def main():
    y = ap(lambda x: x * h0((0 if 19 < 5 else 18), 15) + 1, 4)
    f = lambda x: x * ap(lambda x: x * total(build(5, y)) + 7, pair(13, 13)[1]) + y
    l = build(3, y)
    return (ap(lambda x: x * 6 + 8, pair(10, (18 if y < y else y))[0]), total(build(5, (pair(y, 10)[0] & 6))), f(ap(lambda x: x * 6 + 8, pair(10, (18 if y < y else y))[0])) + f(y), total(l) + total(l))
