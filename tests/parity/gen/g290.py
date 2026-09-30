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
    x = 0
    return ap(lambda x: x * 15 + 2, 19)


def h1(a, b):
    x = pair((total(build(6, b)) if h0(a, 0) < (19 * 3) else a), ap(lambda x: x * 2 + 4, ap(lambda x: x * b + 1, b)))[1]
    return ap(lambda x: x * h0(b, (3 if 10 < 4 else 13)) + 3, (total(build(1, x)) * 12))


def h2(a, b):
    x = 3
    return 16


def main():
    y = h1(pair((8 * 18), 11)[1], h2(ap(lambda x: x * 9 + 3, 4), 7))
    f = lambda x: x * total(build(1, ap(lambda x: x * 19 + 7, 7))) + y
    l = build(5, y)
    return (pair(h0((3 * y), ap(lambda x: x * 17 + 2, 11)), 13)[0], 4, f(pair(h0((3 * y), ap(lambda x: x * 17 + 2, 11)), 13)[0]) + f(y), total(l) + total(l))
