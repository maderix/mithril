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
    x = 6
    return a


def h1(a, b):
    x = total(build(5, ap(lambda x: x * (5 + 16) + 7, b)))
    return (x - total(build(3, total(build(2, 0)))))


def h2(a, b):
    x = 9
    return pair(h1(total(build(4, 8)), ap(lambda x: x * 7 + 1, x)), pair((x if 6 < 4 else a), 5)[0])[0]


def main():
    y = (((12 if 9 < 4 else 19) ^ pair(2, 11)[0]) if (5 - ap(lambda x: x * 15 + 4, 15)) < 8 else 18)
    f = lambda x: x * h0(y, h2(9, y)) + y
    l = build(2, y)
    return (h1(y, ap(lambda x: x * y + 6, (y if 3 < 1 else 14))), (h1(pair(16, y)[0], h0(7, y)) + h1(18, ap(lambda x: x * 5 + 4, y))), f(h1(y, ap(lambda x: x * y + 6, (y if 3 < 1 else 14)))) + f(y), total(l) + total(l))
