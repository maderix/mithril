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
    x = (a if 3 < 13 else 4)
    return (total(build(4, total(build(2, 5)))) + (ap(lambda x: x * b + 4, 1) if (a * 5) < x else 13))


def h1(a, b):
    x = h0(h0(ap(lambda x: x * 16 + 3, a), total(build(6, 17))), (ap(lambda x: x * 1 + 1, b) - pair(16, a)[1]))
    return ((8 if (6 if b < x else a) < ap(lambda x: x * 6 + 4, x) else 14) if 10 < (x * total(build(3, 4))) else ((b if a < b else 6) if h0(12, 14) < (9 - b) else (11 if 19 < b else a)))


def h2(a, b):
    x = (13 if a < ap(lambda x: x * (a + 1) + 0, 8) else total(build(5, b)))
    return total(build(1, h1(total(build(6, x)), 14)))


def main():
    y = (pair((9 & 8), 14)[0] - 5)
    f = lambda x: x * pair((17 if 2 < y else 14), (y if 6 < 15 else 19))[1] + y
    l = build(6, y)
    return ((total(build(0, y)) * y), h0(y, (pair(18, 2)[1] if 10 < total(build(5, 1)) else (y ^ y))), f((total(build(0, y)) * y)) + f(y), total(l) + total(l))
