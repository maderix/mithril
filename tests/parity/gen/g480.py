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
    x = pair((4 if b < pair(b, b)[0] else 13), (b + (a & 8)))[1]
    return (ap(lambda x: x * pair(a, 5)[1] + 5, ap(lambda x: x * a + 1, 18)) if (3 if (b if b < 6 else b) < 10 else pair(b, b)[0]) < 19 else (14 if total(build(5, b)) < b else a))


def h1(a, b):
    x = pair(((5 & b) if 16 < total(build(6, 7)) else pair(16, b)[0]), a)[0]
    return 19


def h2(a, b):
    x = total(build(4, h0(4, h1(a, 17))))
    return pair(((3 ^ a) - total(build(4, a))), (b - pair(19, b)[0]))[1]


def main():
    y = h2(h1((18 * 16), 6), ap(lambda x: x * 5 + 6, pair(7, 2)[1]))
    f = lambda x: x * 6 + y
    l = build(3, y)
    return ((14 if pair(ap(lambda x: x * y + 7, 11), y)[0] < pair(y, pair(y, y)[1])[0] else h2(8, total(build(2, y)))), (h1((y & 4), (6 if y < y else y)) ^ h0((y + y), (y + y))), f((14 if pair(ap(lambda x: x * y + 7, 11), y)[0] < pair(y, pair(y, y)[1])[0] else h2(8, total(build(2, y))))) + f(y), total(l) + total(l))
