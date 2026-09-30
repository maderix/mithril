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
    x = pair(total(build(0, 3)), (pair(b, 16)[1] - total(build(2, a))))[0]
    return 8


def h1(a, b):
    x = ap(lambda x: x * (6 if (a if 6 < 13 else b) < b else ap(lambda x: x * 16 + 8, b)) + 1, 12)
    return 19


def h2(a, b):
    x = pair((pair(9, a)[0] + ap(lambda x: x * 11 + 2, 6)), pair((5 & b), (19 * 6))[0])[1]
    return x


def main():
    y = 6
    f = lambda x: x * (17 if y < ap(lambda x: x * 2 + 2, y) else h2(y, y)) + y
    l = build(2, y)
    return (pair(pair((3 + 12), 5)[1], ((y if 4 < 14 else y) ^ (y if 2 < y else y)))[1], ap(lambda x: x * y + 4, total(build(4, pair(17, y)[1]))), f(pair(pair((3 + 12), 5)[1], ((y if 4 < 14 else y) ^ (y if 2 < y else y)))[1]) + f(y), total(l) + total(l))
