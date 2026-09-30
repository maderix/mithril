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
    x = 7
    return a


def h1(a, b):
    x = a
    return (a if (ap(lambda x: x * a + 8, x) + (19 if 3 < 16 else x)) < h0(h0(b, 16), 2) else h0(h0(b, x), total(build(6, 19))))


def h2(a, b):
    x = 0
    return h1(b, ap(lambda x: x * ap(lambda x: x * x + 7, 6) + 1, pair(15, 16)[1]))


def main():
    y = (6 if (13 ^ (7 ^ 8)) < ((15 & 12) if h2(19, 7) < (9 if 16 < 7 else 0) else 14) else 17)
    f = lambda x: x * (12 ^ ap(lambda x: x * y + 6, y)) + y
    l = build(3, y)
    return (ap(lambda x: x * (h2(15, y) ^ 0) + 3, (ap(lambda x: x * 6 + 2, y) if pair(9, y)[0] < y else (y if y < y else y))), y, f(ap(lambda x: x * (h2(15, y) ^ 0) + 3, (ap(lambda x: x * 6 + 2, y) if pair(9, y)[0] < y else (y if y < y else y)))) + f(y), total(l) + total(l))
