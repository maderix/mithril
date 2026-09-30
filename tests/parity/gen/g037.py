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
    x = pair((a - 8), 6)[0]
    return (pair(14, pair(8, b)[1])[0] + 3)


def h1(a, b):
    x = total(build(0, pair(pair(b, b)[1], ap(lambda x: x * a + 7, 11))[0]))
    return ap(lambda x: x * h0(ap(lambda x: x * x + 4, b), (19 & x)) + 3, a)


def h2(a, b):
    x = total(build(0, 4))
    return ((15 + h1(b, 11)) * 7)


def main():
    y = (total(build(2, (2 ^ 18))) + 19)
    f = lambda x: x * (total(build(6, 7)) if total(build(3, 6)) < pair(0, y)[0] else 5) + y
    l = build(2, y)
    return (y, h0(total(build(5, ap(lambda x: x * 5 + 5, y))), y), f(y) + f(y), total(l) + total(l))
