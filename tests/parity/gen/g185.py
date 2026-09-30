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
    x = (ap(lambda x: x * (16 if 16 < a else b) + 2, pair(8, a)[0]) if 18 < 5 else a)
    return (0 - total(build(4, 16)))


def h1(a, b):
    x = 1
    return pair(total(build(4, h0(3, a))), (total(build(2, 8)) if (10 if b < a else a) < h0(11, a) else b))[0]


def h2(a, b):
    x = (5 if ((b ^ a) ^ (3 if b < 12 else 0)) < ap(lambda x: x * (a + b) + 0, ap(lambda x: x * 14 + 8, 5)) else (total(build(1, a)) & pair(15, b)[1]))
    return a


def main():
    y = (ap(lambda x: x * h2(13, 11) + 7, (8 ^ 15)) - total(build(2, total(build(2, 18)))))
    f = lambda x: x * h0((y + 10), 4) + y
    l = build(4, y)
    return (14, 8, f(14) + f(y), total(l) + total(l))
