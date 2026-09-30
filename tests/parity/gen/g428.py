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
    x = (total(build(6, ap(lambda x: x * b + 5, b))) if 15 < (a - 2) else pair(total(build(0, 7)), 5)[0])
    return ap(lambda x: x * pair((b + 2), (15 if 6 < a else x))[0] + 3, 6)


def h1(a, b):
    x = pair(b, 0)[1]
    return ap(lambda x: x * ap(lambda x: x * h0(b, a) + 6, b) + 4, (pair(a, 13)[0] ^ pair(6, x)[1]))


def h2(a, b):
    x = ap(lambda x: x * pair(total(build(0, b)), pair(2, a)[0])[0] + 0, b)
    return ap(lambda x: x * total(build(2, (a & 9))) + 2, (pair(5, 5)[0] if (4 - 17) < ap(lambda x: x * x + 0, 0) else ap(lambda x: x * 12 + 8, x)))


def main():
    y = total(build(1, ((4 if 13 < 5 else 15) + ap(lambda x: x * 7 + 5, 11))))
    f = lambda x: x * ap(lambda x: x * h2(y, 18) + 2, ap(lambda x: x * 7 + 7, 8)) + y
    l = build(4, y)
    return (pair(3, pair((y - y), pair(19, 3)[0])[1])[0], ((y if h2(14, y) < h1(8, 19) else h2(y, 4)) - pair(17, (y if 5 < y else 19))[0]), f(pair(3, pair((y - y), pair(19, 3)[0])[1])[0]) + f(y), total(l) + total(l))
