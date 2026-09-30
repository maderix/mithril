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
    x = ((18 ^ (18 if a < 12 else a)) if ap(lambda x: x * 8 + 3, (a if a < 4 else a)) < 9 else b)
    return ap(lambda x: x * x + 1, b)


def h1(a, b):
    x = (9 if pair(pair(18, a)[1], (b if b < a else 13))[0] < total(build(2, pair(b, a)[1])) else (17 ^ a))
    return total(build(1, (ap(lambda x: x * b + 3, 12) if 12 < (5 if b < 6 else a) else ap(lambda x: x * x + 4, x))))


def h2(a, b):
    x = (total(build(3, ap(lambda x: x * a + 2, b))) & (h1(a, a) * pair(b, b)[0]))
    return ((b if (0 * a) < ap(lambda x: x * 7 + 4, b) else (10 & 12)) if (7 if pair(x, 12)[0] < (4 if 0 < 3 else x) else ap(lambda x: x * 9 + 8, 13)) < 10 else total(build(6, (x if 16 < a else b))))


def main():
    y = 1
    f = lambda x: x * ap(lambda x: x * total(build(2, y)) + 3, y) + y
    l = build(6, y)
    return (ap(lambda x: x * ap(lambda x: x * pair(y, y)[0] + 2, (y & 9)) + 0, y), ((ap(lambda x: x * 14 + 6, y) & 7) if y < pair((y if 18 < y else y), h1(1, y))[0] else y), f(ap(lambda x: x * ap(lambda x: x * pair(y, y)[0] + 2, (y & 9)) + 0, y)) + f(y), total(l) + total(l))
