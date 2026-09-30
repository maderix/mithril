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
    x = ap(lambda x: x * 7 + 2, (total(build(2, 15)) & (a if 11 < b else a)))
    return (18 if pair(13, total(build(0, 15)))[0] < ((15 + x) ^ (a if b < x else 7)) else (a if total(build(5, 11)) < (b + a) else ap(lambda x: x * a + 5, 15)))


def h1(a, b):
    x = ((b * total(build(1, 19))) if (total(build(4, 3)) if b < h0(18, b) else pair(18, 4)[0]) < total(build(5, a)) else ((5 ^ a) if 1 < a else h0(a, 13)))
    return (ap(lambda x: x * a + 4, h0(a, 0)) & x)


def h2(a, b):
    x = ((total(build(1, b)) if (a - 13) < pair(b, a)[0] else total(build(2, a))) if total(build(5, pair(a, 11)[1])) < ((6 - 3) if b < total(build(5, b)) else ap(lambda x: x * 4 + 8, b)) else total(build(3, (a if a < b else 8))))
    return total(build(3, h0(ap(lambda x: x * x + 5, b), total(build(3, a)))))


def main():
    y = ((ap(lambda x: x * 12 + 0, 17) if 7 < pair(14, 16)[0] else pair(19, 7)[1]) ^ ap(lambda x: x * total(build(5, 11)) + 2, pair(12, 0)[0]))
    f = lambda x: x * h0((y if 12 < 15 else 18), pair(18, 13)[1]) + y
    l = build(5, y)
    return (pair(pair(h0(10, y), h0(15, y))[0], pair((18 if 13 < 6 else y), 16)[1])[1], pair(((3 if 16 < y else 17) - h0(y, y)), h2(ap(lambda x: x * y + 2, y), (8 if 13 < 7 else y)))[0], f(pair(pair(h0(10, y), h0(15, y))[0], pair((18 if 13 < 6 else y), 16)[1])[1]) + f(y), total(l) + total(l))
