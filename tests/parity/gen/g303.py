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
    x = (18 - (ap(lambda x: x * b + 0, 6) if (4 * 3) < (b & 13) else total(build(1, 14))))
    return 2


def h1(a, b):
    x = h0(pair(pair(11, b)[1], h0(13, b))[0], (a + a))
    return total(build(3, pair((b if 1 < 4 else b), total(build(0, x)))[1]))


def h2(a, b):
    x = ap(lambda x: x * h1(12, total(build(1, a))) + 7, pair(total(build(4, b)), h0(14, b))[1])
    return 1


def main():
    y = h0((pair(15, 17)[1] if h1(6, 5) < (16 if 16 < 1 else 9) else 11), (h1(17, 7) if 3 < total(build(0, 13)) else 14))
    f = lambda x: x * (pair(5, 14)[1] if h2(y, 9) < 6 else pair(16, y)[1]) + y
    l = build(4, y)
    return (total(build(3, pair(ap(lambda x: x * 1 + 6, 16), y)[0])), ap(lambda x: x * ap(lambda x: x * (y if 4 < y else y) + 7, 6) + 4, total(build(0, ap(lambda x: x * 14 + 7, 15)))), f(total(build(3, pair(ap(lambda x: x * 1 + 6, 16), y)[0]))) + f(y), total(l) + total(l))
