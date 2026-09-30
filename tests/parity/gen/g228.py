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
    x = 1
    return 14


def h1(a, b):
    x = 8
    return h0(b, (pair(b, b)[0] if (17 if b < a else b) < ap(lambda x: x * 3 + 0, 4) else 4))


def h2(a, b):
    x = h0(total(build(3, h1(a, b))), 0)
    return total(build(4, pair((12 if b < x else 13), h0(a, x))[1]))


def main():
    y = 18
    f = lambda x: x * ap(lambda x: x * total(build(3, 8)) + 2, y) + y
    l = build(1, y)
    return ((h2(10, y) + (y * y)), h2(total(build(2, y)), pair((8 + 2), 11)[0]), f((h2(10, y) + (y * y))) + f(y), total(l) + total(l))
