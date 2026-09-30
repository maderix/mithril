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
    x = 19
    return total(build(0, 7))


def h1(a, b):
    x = ap(lambda x: x * (total(build(3, 10)) & total(build(5, b))) + 2, (4 * (15 if 13 < 14 else b)))
    return h0(ap(lambda x: x * 4 + 3, (x ^ 18)), ap(lambda x: x * ap(lambda x: x * 3 + 2, b) + 5, 5))


def h2(a, b):
    x = ((total(build(4, 3)) ^ pair(4, 3)[0]) if ((13 if 13 < a else b) + pair(15, b)[0]) < pair(ap(lambda x: x * 3 + 5, 10), total(build(4, 8)))[0] else total(build(1, ap(lambda x: x * b + 8, 12))))
    return total(build(3, a))


def main():
    y = 14
    f = lambda x: x * ((8 ^ y) & h2(1, 13)) + y
    l = build(0, y)
    return (4, (ap(lambda x: x * ap(lambda x: x * y + 7, 12) + 4, (y * y)) if (11 if 7 < 9 else ap(lambda x: x * y + 3, 7)) < 17 else y), f(4) + f(y), total(l) + total(l))
