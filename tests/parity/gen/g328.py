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
    x = 14
    return 16


def h1(a, b):
    x = (17 - a)
    return ((19 if total(build(5, 1)) < total(build(0, 15)) else ap(lambda x: x * x + 5, 18)) * (a if h0(4, 14) < (13 * 14) else b))


def h2(a, b):
    x = ap(lambda x: x * 16 + 4, ((0 if 16 < a else b) + 13))
    return 13


def main():
    y = ap(lambda x: x * 17 + 5, total(build(2, (18 & 15))))
    f = lambda x: x * total(build(4, pair(17, y)[1])) + y
    l = build(4, y)
    return (total(build(1, h2(total(build(3, 6)), 17))), (h2(ap(lambda x: x * y + 8, 7), (7 * 1)) if (pair(8, 6)[1] if pair(y, 19)[1] < h0(16, 9) else h1(y, 5)) < ap(lambda x: x * ap(lambda x: x * y + 0, 19) + 8, (y * 0)) else total(build(6, y))), f(total(build(1, h2(total(build(3, 6)), 17)))) + f(y), total(l) + total(l))
