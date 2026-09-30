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
    x = (ap(lambda x: x * ap(lambda x: x * a + 1, a) + 5, 11) & pair(15, ap(lambda x: x * a + 2, 15))[1])
    return (ap(lambda x: x * pair(a, 4)[1] + 2, a) & ap(lambda x: x * ap(lambda x: x * b + 5, b) + 4, pair(13, b)[0]))


def h1(a, b):
    x = total(build(0, h0(pair(11, 17)[0], pair(b, a)[1])))
    return ap(lambda x: x * b + 3, total(build(6, 12)))


def h2(a, b):
    x = total(build(6, (b * (7 - b))))
    return 3


def main():
    y = ((ap(lambda x: x * 6 + 2, 4) * total(build(2, 14))) if (pair(0, 0)[1] ^ ap(lambda x: x * 8 + 8, 16)) < 15 else ap(lambda x: x * total(build(5, 0)) + 2, ap(lambda x: x * 19 + 8, 6)))
    f = lambda x: x * pair(h2(0, y), h1(7, y))[0] + y
    l = build(6, y)
    return ((h0(y, h2(y, 18)) & y), total(build(6, ap(lambda x: x * h0(y, y) + 7, pair(y, y)[0]))), f((h0(y, h2(y, 18)) & y)) + f(y), total(l) + total(l))
