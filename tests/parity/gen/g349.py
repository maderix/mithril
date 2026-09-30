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
    x = 5
    return 16


def h1(a, b):
    x = (total(build(6, h0(11, 10))) if b < ap(lambda x: x * 0 + 6, pair(6, 9)[0]) else 9)
    return h0(ap(lambda x: x * a + 0, total(build(2, x))), 9)


def h2(a, b):
    x = pair(total(build(0, (b if 8 < a else 19))), h0((17 + 18), total(build(4, a))))[1]
    return h1((5 & (x if 13 < a else 18)), ap(lambda x: x * (5 ^ 10) + 8, x))


def main():
    y = ap(lambda x: x * 14 + 6, total(build(5, 16)))
    f = lambda x: x * ((y if 12 < y else 1) if (y if 0 < 4 else y) < pair(3, y)[1] else 17) + y
    l = build(5, y)
    return (total(build(4, ((y - 4) if ap(lambda x: x * 8 + 3, 10) < (10 - 2) else ap(lambda x: x * y + 2, y)))), (y if (pair(5, 18)[0] if total(build(3, y)) < total(build(5, 17)) else y) < h0(total(build(5, y)), pair(19, y)[0]) else ap(lambda x: x * (y + 13) + 3, h1(y, y))), f(total(build(4, ((y - 4) if ap(lambda x: x * 8 + 3, 10) < (10 - 2) else ap(lambda x: x * y + 2, y))))) + f(y), total(l) + total(l))
