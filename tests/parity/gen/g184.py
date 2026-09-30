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
    x = pair(b, 15)[0]
    return total(build(4, (pair(6, x)[1] if pair(x, a)[1] < 19 else (a ^ a))))


def h1(a, b):
    x = 4
    return total(build(4, (10 if (4 if 10 < 10 else b) < ap(lambda x: x * 9 + 7, x) else 11)))


def h2(a, b):
    x = ap(lambda x: x * h1((19 ^ 8), h0(a, 8)) + 7, ((4 + a) if pair(18, b)[1] < (b + b) else total(build(1, 9))))
    return 9


def main():
    y = total(build(3, (ap(lambda x: x * 1 + 1, 17) if pair(1, 17)[0] < pair(9, 4)[0] else h2(2, 1))))
    f = lambda x: x * pair(h1(y, 8), pair(19, 11)[0])[0] + y
    l = build(0, y)
    return ((ap(lambda x: x * ap(lambda x: x * 18 + 5, 10) + 2, h1(14, 18)) if ap(lambda x: x * h0(2, 2) + 8, (18 if 17 < y else y)) < 1 else ((y - 17) if total(build(5, 4)) < ap(lambda x: x * y + 2, y) else pair(4, 12)[0])), (8 if total(build(0, pair(1, 9)[1])) < h1(0, (13 if 7 < y else y)) else ((y if 17 < 3 else 6) if (12 ^ y) < (17 - 11) else (2 if 7 < y else 19))), f((ap(lambda x: x * ap(lambda x: x * 18 + 5, 10) + 2, h1(14, 18)) if ap(lambda x: x * h0(2, 2) + 8, (18 if 17 < y else y)) < 1 else ((y - 17) if total(build(5, 4)) < ap(lambda x: x * y + 2, y) else pair(4, 12)[0]))) + f(y), total(l) + total(l))
