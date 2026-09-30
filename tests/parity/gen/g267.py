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
    x = 13
    return 6


def h1(a, b):
    x = h0(ap(lambda x: x * 19 + 4, ap(lambda x: x * a + 4, 5)), h0(h0(b, b), b))
    return b


def h2(a, b):
    x = ap(lambda x: x * 17 + 8, h0(ap(lambda x: x * b + 7, a), (14 if 12 < a else b)))
    return (ap(lambda x: x * h0(b, b) + 2, (b ^ x)) if (h0(b, 13) + (10 if 3 < x else x)) < (h1(b, 15) - total(build(3, x))) else (pair(b, 1)[0] + (4 if 16 < a else x)))


def main():
    y = 10
    f = lambda x: x * (h2(9, 8) if total(build(2, 17)) < h0(11, 9) else (y if 10 < 7 else 6)) + y
    l = build(4, y)
    return (ap(lambda x: x * y + 1, (pair(y, y)[0] - h1(18, y))), total(build(1, total(build(1, pair(y, y)[1])))), f(ap(lambda x: x * y + 1, (pair(y, y)[0] - h1(18, y)))) + f(y), total(l) + total(l))
