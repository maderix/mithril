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
    x = (11 if ((b - 17) if total(build(4, a)) < 16 else pair(a, 6)[1]) < (a ^ (18 - 15)) else (ap(lambda x: x * a + 4, 0) + 8))
    return 15


def h1(a, b):
    x = ap(lambda x: x * 13 + 5, (h0(1, a) if (18 if 5 < 10 else a) < (14 if a < b else 15) else ap(lambda x: x * 18 + 2, 12)))
    return pair((h0(17, a) if (a + 18) < h0(a, 0) else (8 * 11)), b)[1]


def h2(a, b):
    x = h1(total(build(4, pair(0, a)[1])), (pair(b, 15)[1] + (b if 6 < 15 else b)))
    return total(build(2, total(build(0, 1))))


def main():
    y = (total(build(6, 18)) if 10 < 3 else ((5 if 12 < 12 else 1) ^ 0))
    f = lambda x: x * (h0(17, 18) - ap(lambda x: x * y + 8, y)) + y
    l = build(5, y)
    return (16, y, f(16) + f(y), total(l) + total(l))
