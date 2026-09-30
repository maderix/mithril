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
    x = (pair(8, pair(14, a)[0])[1] - ap(lambda x: x * 0 + 1, (5 ^ 2)))
    return a


def h1(a, b):
    x = pair(ap(lambda x: x * (1 if a < a else b) + 2, (a if 12 < b else b)), (ap(lambda x: x * 12 + 2, b) if h0(b, 1) < pair(b, b)[0] else total(build(4, 11))))[0]
    return 3


def h2(a, b):
    x = 1
    return total(build(0, pair((a if a < b else a), (a if 15 < 3 else 11))[1]))


def main():
    y = (pair(14, 17)[1] if (h1(6, 12) if h0(11, 9) < pair(5, 18)[1] else (10 + 17)) < (12 * h2(19, 19)) else (h2(6, 8) if 6 < h2(6, 3) else (14 if 15 < 17 else 12)))
    f = lambda x: x * ap(lambda x: x * h2(y, 18) + 2, 14) + y
    l = build(3, y)
    return ((y if 8 < pair(y, pair(y, 13)[0])[0] else ap(lambda x: x * (y if y < y else 19) + 3, pair(12, y)[0])), total(build(5, 10)), f((y if 8 < pair(y, pair(y, 13)[0])[0] else ap(lambda x: x * (y if y < y else 19) + 3, pair(12, y)[0]))) + f(y), total(l) + total(l))
