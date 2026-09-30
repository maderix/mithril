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
    x = pair(5, 5)[0]
    return 12


def h1(a, b):
    x = 0
    return (total(build(5, (15 & x))) if ap(lambda x: x * ap(lambda x: x * 4 + 3, 6) + 0, h0(12, 0)) < h0(a, pair(a, a)[0]) else pair(pair(b, b)[0], pair(x, a)[0])[1])


def h2(a, b):
    x = 10
    return b


def main():
    y = (5 if h2(h0(10, 4), 18) < (total(build(6, 16)) - ap(lambda x: x * 5 + 7, 19)) else 6)
    f = lambda x: x * h0((9 if y < y else 11), h0(7, y)) + y
    l = build(2, y)
    return (total(build(5, y)), ap(lambda x: x * pair((3 ^ 16), (y ^ 2))[0] + 6, total(build(0, ap(lambda x: x * y + 2, 5)))), f(total(build(5, y))) + f(y), total(l) + total(l))
