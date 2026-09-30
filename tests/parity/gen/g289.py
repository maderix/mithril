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
    x = ((ap(lambda x: x * a + 6, a) + 2) + (total(build(2, a)) if (7 - 7) < total(build(4, b)) else (a * 10)))
    return ap(lambda x: x * 10 + 5, pair((b if 19 < b else 0), 12)[1])


def h1(a, b):
    x = (total(build(0, (a if b < 16 else a))) ^ total(build(0, (1 & 0))))
    return pair((11 * (x ^ 13)), 6)[1]


def h2(a, b):
    x = (pair(h1(2, 6), ap(lambda x: x * a + 6, 0))[0] + h1(15, a))
    return (total(build(0, 3)) + pair((7 if x < b else 1), total(build(4, x)))[1])


def main():
    y = (pair(ap(lambda x: x * 17 + 1, 18), h0(2, 18))[0] + ap(lambda x: x * 16 + 7, (0 if 14 < 8 else 2)))
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * 6 + 5, 11) + 1, (18 & y)) + y
    l = build(1, y)
    return ((h1((y & y), 2) if h2((13 if 14 < 13 else 14), pair(5, y)[0]) < ((4 if 0 < y else y) - ap(lambda x: x * 19 + 0, 11)) else ap(lambda x: x * pair(y, 2)[1] + 4, (y if 3 < y else y))), 7, f((h1((y & y), 2) if h2((13 if 14 < 13 else 14), pair(5, y)[0]) < ((4 if 0 < y else y) - ap(lambda x: x * 19 + 0, 11)) else ap(lambda x: x * pair(y, 2)[1] + 4, (y if 3 < y else y)))) + f(y), total(l) + total(l))
