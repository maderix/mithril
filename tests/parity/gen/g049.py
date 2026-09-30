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
    x = ap(lambda x: x * 16 + 7, ap(lambda x: x * ap(lambda x: x * b + 4, b) + 5, total(build(2, 12))))
    return pair(total(build(0, (x + a))), pair(19, pair(6, 1)[0])[1])[0]


def h1(a, b):
    x = 14
    return (ap(lambda x: x * pair(b, a)[1] + 8, (1 + a)) if total(build(0, 18)) < pair(pair(15, 1)[1], ap(lambda x: x * b + 0, x))[1] else 1)


def h2(a, b):
    x = h1((a & ap(lambda x: x * b + 8, 18)), (12 + total(build(6, 12))))
    return 2


def main():
    y = 11
    f = lambda x: x * pair(y, pair(y, y)[0])[1] + y
    l = build(5, y)
    return (6, h1(ap(lambda x: x * y + 7, h0(7, 16)), ap(lambda x: x * 13 + 2, h1(y, y))), f(6) + f(y), total(l) + total(l))
