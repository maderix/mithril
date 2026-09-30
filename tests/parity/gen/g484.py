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
    x = (total(build(3, ap(lambda x: x * a + 0, 0))) - 7)
    return (pair(pair(13, a)[1], ap(lambda x: x * 15 + 1, 0))[0] & pair(15, pair(b, 10)[1])[0])


def h1(a, b):
    x = (b if pair((b - 10), h0(9, b))[1] < a else (ap(lambda x: x * b + 4, 5) if 15 < pair(6, 9)[0] else pair(15, 5)[0]))
    return total(build(4, ap(lambda x: x * ap(lambda x: x * a + 3, 13) + 3, pair(7, x)[1])))


def h2(a, b):
    x = b
    return b


def main():
    y = ap(lambda x: x * h2((16 if 3 < 10 else 1), h0(7, 9)) + 7, pair(pair(1, 10)[0], ap(lambda x: x * 9 + 6, 17))[1])
    f = lambda x: x * pair(10, 0)[0] + y
    l = build(6, y)
    return (y, (ap(lambda x: x * y + 5, (18 if 12 < 5 else 4)) - (total(build(4, 4)) if y < y else (y if 8 < 4 else y))), f(y) + f(y), total(l) + total(l))
