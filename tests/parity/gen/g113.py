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
    x = pair(3, total(build(2, 4)))[0]
    return (pair((17 - a), total(build(5, 11)))[1] + total(build(0, pair(13, 4)[1])))


def h1(a, b):
    x = b
    return (ap(lambda x: x * total(build(0, 13)) + 4, (x if 13 < a else 3)) if total(build(5, pair(a, a)[1])) < ap(lambda x: x * pair(19, a)[1] + 7, pair(11, a)[0]) else a)


def h2(a, b):
    x = h0(pair(pair(a, 13)[1], (b ^ b))[1], (a ^ h1(5, a)))
    return ap(lambda x: x * total(build(4, b)) + 7, (19 if (0 if x < 5 else a) < h0(x, x) else a))


def main():
    y = pair(11, (10 if pair(5, 1)[0] < ap(lambda x: x * 17 + 4, 7) else (9 - 3)))[1]
    f = lambda x: x * ap(lambda x: x * (y if 12 < y else 2) + 3, h0(y, 1)) + y
    l = build(6, y)
    return (y, 14, f(y) + f(y), total(l) + total(l))
