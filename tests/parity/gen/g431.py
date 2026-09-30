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
    x = ap(lambda x: x * ap(lambda x: x * ap(lambda x: x * a + 8, 1) + 0, ap(lambda x: x * 13 + 0, 0)) + 7, 16)
    return 4


def h1(a, b):
    x = 8
    return b


def h2(a, b):
    x = total(build(1, 12))
    return 2


def main():
    y = h0(13, h1(total(build(5, 19)), ap(lambda x: x * 7 + 7, 14)))
    f = lambda x: x * pair(y, y)[1] + y
    l = build(0, y)
    return (pair(((y if y < y else y) - (11 if y < 5 else 1)), h0(0, 0))[1], h1(total(build(4, y)), (ap(lambda x: x * 5 + 5, y) & y)), f(pair(((y if y < y else y) - (11 if y < 5 else 1)), h0(0, 0))[1]) + f(y), total(l) + total(l))
