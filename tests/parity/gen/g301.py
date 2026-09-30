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
    x = 14
    return (pair(5, (x if 7 < b else 2))[0] if 5 < total(build(6, 5)) else ((8 - 13) if (a if a < 9 else 9) < (x - 14) else pair(12, 14)[0]))


def h1(a, b):
    x = ((h0(18, 2) if ap(lambda x: x * b + 0, 0) < a else (1 if b < 13 else 14)) if 15 < (b * 12) else 8)
    return pair(total(build(2, (b if 0 < b else x))), ap(lambda x: x * total(build(5, x)) + 4, total(build(5, x))))[1]


def h2(a, b):
    x = pair(pair(16, a)[0], 13)[1]
    return h1(total(build(4, a)), total(build(3, ap(lambda x: x * 1 + 1, x))))


def main():
    y = 18
    f = lambda x: x * total(build(6, (y ^ 11))) + y
    l = build(1, y)
    return (pair((19 - y), 1)[1], (15 ^ pair(ap(lambda x: x * 2 + 7, 2), (15 if 6 < 9 else 10))[0]), f(pair((19 - y), 1)[1]) + f(y), total(l) + total(l))
