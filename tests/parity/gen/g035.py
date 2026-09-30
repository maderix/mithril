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
    x = (19 - 18)
    return (pair(b, b)[0] - ((x if 16 < x else a) if total(build(4, 12)) < (7 - b) else (3 if b < 15 else a)))


def h1(a, b):
    x = b
    return ap(lambda x: x * (pair(b, a)[1] if 10 < ap(lambda x: x * b + 8, b) else (x if 13 < 18 else 18)) + 6, ap(lambda x: x * pair(x, a)[1] + 3, 14))


def h2(a, b):
    x = h0(pair(pair(b, a)[0], pair(a, 7)[1])[1], h1((11 - 7), a))
    return pair(x, x)[1]


def main():
    y = h2(pair(1, h0(1, 16))[1], ap(lambda x: x * total(build(1, 12)) + 0, (13 & 14)))
    f = lambda x: x * h0(ap(lambda x: x * 14 + 3, y), total(build(3, 6))) + y
    l = build(2, y)
    return (ap(lambda x: x * ap(lambda x: x * total(build(3, y)) + 5, (18 ^ 7)) + 7, ap(lambda x: x * y + 6, (19 - 15))), 13, f(ap(lambda x: x * ap(lambda x: x * total(build(3, y)) + 5, (18 ^ 7)) + 7, ap(lambda x: x * y + 6, (19 - 15)))) + f(y), total(l) + total(l))
