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
    x = 4
    return 9


def h1(a, b):
    x = total(build(1, (17 if a < ap(lambda x: x * 11 + 3, 9) else ap(lambda x: x * 1 + 1, a))))
    return ap(lambda x: x * pair(1, ap(lambda x: x * x + 8, x))[0] + 5, pair(pair(12, 12)[1], 8)[1])


def h2(a, b):
    x = b
    return total(build(0, h0((12 - 10), (7 if a < x else x))))


def main():
    y = (((10 & 14) - ap(lambda x: x * 16 + 8, 4)) & h2(pair(17, 15)[0], 12))
    f = lambda x: x * y + y
    l = build(1, y)
    return (h2((5 + y), (pair(7, 7)[0] if (y if 19 < 7 else 3) < ap(lambda x: x * y + 8, 0) else pair(1, y)[1])), 5, f(h2((5 + y), (pair(7, 7)[0] if (y if 19 < 7 else 3) < ap(lambda x: x * y + 8, 0) else pair(1, y)[1]))) + f(y), total(l) + total(l))
