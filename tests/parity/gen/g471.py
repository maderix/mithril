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
    x = total(build(0, pair((7 if b < 3 else a), total(build(4, a)))[1]))
    return 0


def h1(a, b):
    x = ap(lambda x: x * total(build(5, pair(a, 0)[1])) + 0, ap(lambda x: x * h0(b, 15) + 6, 14))
    return h0(pair(b, pair(14, b)[1])[1], total(build(3, (b + 13))))


def h2(a, b):
    x = a
    return a


def main():
    y = pair(16, (8 + pair(13, 2)[1]))[1]
    f = lambda x: x * h1(total(build(5, 2)), ap(lambda x: x * 3 + 8, 17)) + y
    l = build(5, y)
    return (17, pair((h1(y, y) if h0(y, y) < pair(y, 5)[0] else y), pair(ap(lambda x: x * 19 + 6, 12), (2 + 6))[1])[0], f(17) + f(y), total(l) + total(l))
