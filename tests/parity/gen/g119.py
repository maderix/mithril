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
    return (total(build(4, b)) & ap(lambda x: x * (x if 19 < a else 3) + 5, pair(a, a)[1]))


def h1(a, b):
    x = pair((pair(17, b)[1] if b < 11 else a), 2)[0]
    return pair(total(build(2, total(build(6, 2)))), ap(lambda x: x * ap(lambda x: x * 14 + 4, a) + 1, total(build(0, 4))))[0]


def h2(a, b):
    x = h1(((14 if 15 < b else 15) * a), a)
    return (total(build(3, pair(b, 14)[1])) - total(build(4, 1)))


def main():
    y = (10 if h0(9, ap(lambda x: x * 19 + 0, 4)) < (pair(9, 11)[1] ^ total(build(4, 12))) else total(build(6, h0(18, 14))))
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * 16 + 7, 13) + 6, (y ^ y)) + y
    l = build(4, y)
    return ((h1(y, pair(1, y)[0]) * h2(h2(y, 18), h2(18, 4))), ((pair(y, 14)[0] + (y - 15)) + y), f((h1(y, pair(1, y)[0]) * h2(h2(y, 18), h2(18, 4)))) + f(y), total(l) + total(l))
