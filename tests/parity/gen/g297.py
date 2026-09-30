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
    x = ap(lambda x: x * 1 + 0, (a if total(build(5, a)) < 3 else (b if 3 < b else a)))
    return 2


def h1(a, b):
    x = ap(lambda x: x * h0(pair(a, 14)[0], (a & 7)) + 6, pair(pair(0, 4)[1], pair(3, 1)[1])[1])
    return ((19 if (17 * 5) < pair(b, 13)[1] else (1 ^ a)) if (total(build(5, x)) if (b + 18) < (x * 1) else a) < ap(lambda x: x * ap(lambda x: x * b + 6, 7) + 8, ap(lambda x: x * 1 + 5, x)) else ((a * 9) * (a ^ 2)))


def h2(a, b):
    x = (a if (a if 19 < pair(11, 5)[0] else total(build(5, a))) < ap(lambda x: x * b + 3, pair(13, 9)[1]) else ap(lambda x: x * a + 0, b))
    return x


def main():
    y = pair(ap(lambda x: x * pair(14, 7)[1] + 6, ap(lambda x: x * 11 + 5, 16)), 0)[1]
    f = lambda x: x * total(build(3, y)) + y
    l = build(1, y)
    return (total(build(1, y)), (8 - y), f(total(build(1, y))) + f(y), total(l) + total(l))
