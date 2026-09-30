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
    x = (5 if (total(build(3, a)) if (17 & b) < ap(lambda x: x * b + 5, b) else pair(11, 0)[0]) < 13 else ap(lambda x: x * b + 4, (a if 7 < a else 11)))
    return 4


def h1(a, b):
    x = pair(pair((13 if b < a else b), total(build(3, b)))[0], pair(h0(b, b), h0(15, 14))[0])[1]
    return b


def h2(a, b):
    x = b
    return (6 if (1 + total(build(2, x))) < ((7 if b < 16 else 0) * ap(lambda x: x * a + 8, 19)) else 1)


def main():
    y = pair(h1(h2(14, 13), total(build(0, 6))), total(build(6, (1 if 19 < 1 else 6))))[0]
    f = lambda x: x * total(build(2, (y if y < 10 else y))) + y
    l = build(3, y)
    return (y, (total(build(4, pair(0, y)[1])) if (y ^ (11 if y < 4 else 9)) < (pair(18, y)[0] & y) else total(build(6, (16 + 8)))), f(y) + f(y), total(l) + total(l))
