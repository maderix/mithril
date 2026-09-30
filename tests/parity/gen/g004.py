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
    x = ap(lambda x: x * 19 + 8, 14)
    return 12


def h1(a, b):
    x = (total(build(3, total(build(1, 6)))) if total(build(5, (12 if a < 0 else 8))) < ((0 & a) ^ 6) else pair(h0(4, 2), (7 if b < 19 else a))[0])
    return total(build(3, h0(x, total(build(1, b)))))


def h2(a, b):
    x = (b ^ (19 if a < 5 else ap(lambda x: x * b + 0, b)))
    return (total(build(4, pair(b, b)[1])) if 13 < (h1(a, 3) - (0 if a < 5 else 19)) else h0(ap(lambda x: x * 12 + 1, 16), ap(lambda x: x * a + 6, a)))


def main():
    y = (6 if (ap(lambda x: x * 13 + 2, 18) if pair(16, 19)[0] < 13 else (6 if 12 < 7 else 10)) < (11 & (8 + 1)) else (total(build(4, 5)) if 5 < pair(1, 12)[0] else 5))
    f = lambda x: x * y + y
    l = build(0, y)
    return ((h1(pair(9, y)[1], (16 if y < y else 18)) if h1(h0(1, y), pair(y, 4)[0]) < ((y * y) if ap(lambda x: x * y + 4, 19) < pair(y, 6)[0] else (y * 1)) else ap(lambda x: x * (13 if y < 9 else 1) + 0, (9 if y < 14 else y))), (y if 2 < pair(h0(6, 18), pair(12, 5)[0])[0] else pair(ap(lambda x: x * y + 3, y), 8)[1]), f((h1(pair(9, y)[1], (16 if y < y else 18)) if h1(h0(1, y), pair(y, 4)[0]) < ((y * y) if ap(lambda x: x * y + 4, 19) < pair(y, 6)[0] else (y * 1)) else ap(lambda x: x * (13 if y < 9 else 1) + 0, (9 if y < 14 else y)))) + f(y), total(l) + total(l))
