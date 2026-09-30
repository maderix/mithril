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
    return 5


def h1(a, b):
    x = ap(lambda x: x * pair(h0(10, b), (a if b < 9 else 5))[0] + 5, (h0(b, b) if (b if 14 < 14 else a) < 10 else a))
    return (17 * (h0(9, 0) if 9 < pair(12, 18)[1] else pair(x, 3)[0]))


def h2(a, b):
    x = a
    return (ap(lambda x: x * (a & 2) + 2, pair(7, 10)[0]) & h0(ap(lambda x: x * 1 + 8, x), 16))


def main():
    y = 11
    f = lambda x: x * total(build(4, total(build(5, 7)))) + y
    l = build(6, y)
    return ((pair(y, 16)[1] & h1(ap(lambda x: x * 16 + 2, 9), y)), ap(lambda x: x * 17 + 2, y), f((pair(y, 16)[1] & h1(ap(lambda x: x * 16 + 2, 9), y))) + f(y), total(l) + total(l))
