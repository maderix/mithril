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
    x = b
    return 19


def h1(a, b):
    x = (12 if pair(8, (19 * 13))[0] < (pair(b, 6)[1] - 0) else ap(lambda x: x * a + 8, pair(a, 1)[0]))
    return pair(((a - 0) if a < (18 if 9 < b else a) else 8), pair(pair(3, 19)[1], total(build(3, 10)))[0])[1]


def h2(a, b):
    x = h1(h0(h1(17, 14), total(build(3, b))), total(build(1, a)))
    return (pair((a if x < 7 else 7), (4 if b < a else a))[1] if 5 < 11 else ap(lambda x: x * x + 0, (a if b < 17 else b)))


def main():
    y = ap(lambda x: x * 7 + 8, pair((8 ^ 12), (18 if 7 < 15 else 16))[0])
    f = lambda x: x * total(build(4, ap(lambda x: x * y + 6, y))) + y
    l = build(2, y)
    return (y, h0(((8 - 17) if total(build(2, y)) < h0(y, 12) else pair(y, 1)[0]), ap(lambda x: x * (10 + y) + 4, (y if 3 < y else y))), f(y) + f(y), total(l) + total(l))
