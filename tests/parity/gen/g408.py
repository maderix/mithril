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
    x = ((3 & (b if b < 2 else a)) if pair((1 * a), (b & 8))[0] < a else b)
    return (((b * a) + ap(lambda x: x * 16 + 1, 18)) + (x - 1))


def h1(a, b):
    x = 18
    return total(build(5, a))


def h2(a, b):
    x = 18
    return (pair(pair(9, 15)[0], total(build(5, 4)))[0] * (2 + total(build(5, 17))))


def main():
    y = ((2 if (0 * 0) < (4 if 18 < 4 else 1) else (17 ^ 11)) + ((13 if 1 < 16 else 2) * (12 if 3 < 5 else 6)))
    f = lambda x: x * h1(total(build(1, y)), pair(y, 18)[0]) + y
    l = build(4, y)
    return ((y if (y if pair(y, 15)[1] < total(build(5, y)) else (11 if 10 < y else y)) < total(build(5, total(build(4, y)))) else pair(6, ap(lambda x: x * y + 8, y))[0]), (y - ap(lambda x: x * (y * 14) + 6, (2 if 9 < 7 else 2))), f((y if (y if pair(y, 15)[1] < total(build(5, y)) else (11 if 10 < y else y)) < total(build(5, total(build(4, y)))) else pair(6, ap(lambda x: x * y + 8, y))[0])) + f(y), total(l) + total(l))
