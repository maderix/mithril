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
    return 8


def h1(a, b):
    x = 12
    return x


def h2(a, b):
    x = (h0(b, ap(lambda x: x * 17 + 4, b)) if 1 < ap(lambda x: x * ap(lambda x: x * 3 + 6, 12) + 2, (b ^ a)) else h1(4, 2))
    return x


def main():
    y = pair((h1(2, 14) + total(build(5, 12))), ap(lambda x: x * ap(lambda x: x * 9 + 2, 11) + 4, ap(lambda x: x * 16 + 6, 11)))[0]
    f = lambda x: x * total(build(6, total(build(6, 3)))) + y
    l = build(0, y)
    return (6, h0((h2(y, 1) ^ ap(lambda x: x * y + 2, 16)), 6), f(6) + f(y), total(l) + total(l))
