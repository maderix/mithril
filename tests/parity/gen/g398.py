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
    x = total(build(1, ((0 if 5 < b else a) if (10 * 5) < pair(a, b)[0] else pair(b, 15)[0])))
    return 8


def h1(a, b):
    x = total(build(2, pair((a if 0 < b else 19), (18 * 12))[1]))
    return pair(ap(lambda x: x * (a if 16 < 19 else 3) + 8, pair(0, x)[0]), h0((a * b), 14))[0]


def h2(a, b):
    x = (a + h0(pair(b, 5)[1], h1(18, 14)))
    return ((h0(3, x) if 15 < b else total(build(5, 4))) & 3)


def main():
    y = (pair(pair(15, 5)[0], h0(15, 13))[1] if (6 + pair(7, 16)[1]) < 1 else pair(14, (10 - 14))[0])
    f = lambda x: x * y + y
    l = build(1, y)
    return (ap(lambda x: x * 17 + 8, total(build(5, total(build(6, 19))))), 3, f(ap(lambda x: x * 17 + 8, total(build(5, total(build(6, 19)))))) + f(y), total(l) + total(l))
