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
    x = (a if 18 < pair(a, pair(a, 16)[1])[1] else 10)
    return ap(lambda x: x * (x if 12 < 3 else ap(lambda x: x * 13 + 3, x)) + 0, (total(build(3, 17)) if pair(9, 10)[1] < (9 ^ a) else (b * 9)))


def h1(a, b):
    x = 3
    return h0(total(build(0, h0(x, 5))), ap(lambda x: x * total(build(5, 9)) + 7, 3))


def h2(a, b):
    x = h1((h1(b, a) ^ 6), (total(build(1, a)) if (a * b) < pair(18, 13)[0] else 7))
    return ap(lambda x: x * 10 + 5, (h1(0, 1) if h0(5, 1) < (a if 12 < 7 else 17) else ap(lambda x: x * a + 5, x)))


def main():
    y = pair((pair(2, 3)[1] if h2(12, 6) < ap(lambda x: x * 14 + 5, 4) else pair(6, 14)[0]), (pair(13, 9)[0] if (5 if 15 < 3 else 9) < (14 & 14) else 11))[0]
    f = lambda x: x * ((y + y) ^ (19 if 6 < y else y)) + y
    l = build(4, y)
    return (h2((pair(y, 16)[0] if ap(lambda x: x * y + 5, 3) < (15 if 3 < y else 4) else 2), y), ap(lambda x: x * 2 + 6, (ap(lambda x: x * 15 + 6, y) if (y if 8 < y else y) < pair(y, 4)[1] else (y + y))), f(h2((pair(y, 16)[0] if ap(lambda x: x * y + 5, 3) < (15 if 3 < y else 4) else 2), y)) + f(y), total(l) + total(l))
