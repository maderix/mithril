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
    return (ap(lambda x: x * ap(lambda x: x * 3 + 1, 15) + 6, total(build(0, 7))) & pair((3 if 0 < b else x), (x & b))[0])


def h1(a, b):
    x = (h0((9 if b < a else a), 6) & a)
    return (b & 5)


def h2(a, b):
    x = ap(lambda x: x * total(build(4, (11 & 2))) + 3, ap(lambda x: x * ap(lambda x: x * 12 + 2, 5) + 5, (18 * 13)))
    return 8


def main():
    y = total(build(1, 1))
    f = lambda x: x * y + y
    l = build(4, y)
    return ((16 - 12), h1(((10 if y < y else y) & h2(y, y)), h1(h2(5, y), h0(y, 6))), f((16 - 12)) + f(y), total(l) + total(l))
