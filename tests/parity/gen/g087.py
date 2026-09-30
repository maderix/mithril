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
    x = ap(lambda x: x * (2 & total(build(0, 17))) + 6, 19)
    return (total(build(4, pair(1, x)[1])) * 7)


def h1(a, b):
    x = b
    return (h0((b - a), 0) - h0(b, pair(11, x)[1]))


def h2(a, b):
    x = (a ^ ap(lambda x: x * (19 & a) + 4, 19))
    return ((13 if pair(12, 16)[0] < a else a) & total(build(0, ap(lambda x: x * 5 + 4, b))))


def main():
    y = 14
    f = lambda x: x * 18 + y
    l = build(4, y)
    return (y, pair(y, y)[1], f(y) + f(y), total(l) + total(l))
