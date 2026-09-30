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
    x = pair((18 if 0 < total(build(4, a)) else a), (a + 9))[0]
    return total(build(4, ap(lambda x: x * ap(lambda x: x * b + 1, 7) + 7, (a + x))))


def h1(a, b):
    x = pair(a, ((4 ^ 10) - (0 - 13)))[1]
    return 12


def h2(a, b):
    x = 12
    return pair(7, total(build(2, (x ^ 13))))[1]


def main():
    y = total(build(1, h1(pair(16, 16)[1], (3 ^ 11))))
    f = lambda x: x * ap(lambda x: x * pair(y, y)[1] + 1, h1(y, y)) + y
    l = build(1, y)
    return (((y + h2(6, y)) if pair(pair(0, 14)[1], h2(y, y))[0] < ap(lambda x: x * ap(lambda x: x * 0 + 5, 1) + 7, pair(18, y)[1]) else 5), pair(y, (h0(y, 3) * h0(14, 10)))[0], f(((y + h2(6, y)) if pair(pair(0, 14)[1], h2(y, y))[0] < ap(lambda x: x * ap(lambda x: x * 0 + 5, 1) + 7, pair(18, y)[1]) else 5)) + f(y), total(l) + total(l))
