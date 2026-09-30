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
    x = a
    return (b if total(build(6, 2)) < pair(16, ap(lambda x: x * b + 3, 5))[0] else x)


def h1(a, b):
    x = ap(lambda x: x * (ap(lambda x: x * 3 + 0, b) if 18 < total(build(2, 7)) else a) + 5, b)
    return pair(((b if 17 < 9 else 2) & (1 if x < 5 else b)), (10 * (x if x < 13 else b)))[1]


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * ap(lambda x: x * 10 + 0, b) + 8, (a ^ 4)) + 3, total(build(6, total(build(2, b)))))
    return ((h1(b, a) if b < a else (0 & b)) - total(build(0, b)))


def main():
    y = 11
    f = lambda x: x * y + y
    l = build(4, y)
    return ((ap(lambda x: x * ap(lambda x: x * 7 + 0, 16) + 1, h0(11, y)) * (total(build(5, y)) if ap(lambda x: x * 15 + 0, y) < 4 else 7)), 18, f((ap(lambda x: x * ap(lambda x: x * 7 + 0, 16) + 1, h0(11, y)) * (total(build(5, y)) if ap(lambda x: x * 15 + 0, y) < 4 else 7))) + f(y), total(l) + total(l))
