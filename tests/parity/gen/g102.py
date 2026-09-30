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
    x = pair(ap(lambda x: x * pair(b, a)[0] + 5, 9), 3)[0]
    return ap(lambda x: x * (14 & (11 if 13 < 2 else x)) + 1, 0)


def h1(a, b):
    x = (pair(b, pair(15, 5)[1])[1] if ((b & 17) if ap(lambda x: x * 9 + 8, a) < ap(lambda x: x * 16 + 0, 16) else pair(8, b)[0]) < 14 else ((19 if 8 < b else 11) + total(build(2, a))))
    return 17


def h2(a, b):
    x = b
    return ap(lambda x: x * h0((14 & 3), total(build(1, 14))) + 5, 14)


def main():
    y = (ap(lambda x: x * (6 + 4) + 0, total(build(3, 7))) if ap(lambda x: x * pair(9, 9)[1] + 7, 18) < ((1 + 15) if total(build(5, 1)) < pair(6, 6)[0] else 11) else (h2(6, 5) if (9 if 9 < 14 else 9) < (8 if 2 < 13 else 18) else pair(9, 9)[0]))
    f = lambda x: x * 18 + y
    l = build(1, y)
    return (pair((5 if pair(y, 15)[0] < pair(7, 7)[1] else ap(lambda x: x * 0 + 8, y)), 12)[1], h1((y if (y if y < y else y) < ap(lambda x: x * 19 + 5, 5) else pair(y, y)[0]), h1(15, pair(18, y)[0])), f(pair((5 if pair(y, 15)[0] < pair(7, 7)[1] else ap(lambda x: x * 0 + 8, y)), 12)[1]) + f(y), total(l) + total(l))
