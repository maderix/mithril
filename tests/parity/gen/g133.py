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
    x = 8
    return 15


def h1(a, b):
    x = 17
    return pair(ap(lambda x: x * 16 + 3, a), h0((2 ^ a), h0(3, 5)))[0]


def h2(a, b):
    x = pair(6, (1 if pair(b, b)[1] < h0(0, 3) else h1(10, 14)))[0]
    return 3


def main():
    y = ap(lambda x: x * ap(lambda x: x * 14 + 8, total(build(2, 19))) + 1, 11)
    f = lambda x: x * (y if (11 + 8) < h1(y, 17) else ap(lambda x: x * 16 + 8, y)) + y
    l = build(4, y)
    return (y, ap(lambda x: x * (5 & y) + 0, 13), f(y) + f(y), total(l) + total(l))
