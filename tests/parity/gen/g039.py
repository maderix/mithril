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
    x = ap(lambda x: x * a + 1, pair(a, 8)[0])
    return 8


def h1(a, b):
    x = 1
    return (b if ap(lambda x: x * (a * b) + 4, (5 & a)) < total(build(4, total(build(5, 2)))) else 2)


def h2(a, b):
    x = 4
    return b


def main():
    y = h1(3, (6 ^ 11))
    f = lambda x: x * (h2(13, y) if (10 * y) < h2(y, 8) else pair(3, 17)[0]) + y
    l = build(3, y)
    return (y, (h2(17, ap(lambda x: x * y + 2, 13)) + y), f(y) + f(y), total(l) + total(l))
