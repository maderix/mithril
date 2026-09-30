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
    x = (ap(lambda x: x * 9 + 5, total(build(0, 8))) if pair(pair(a, b)[0], pair(a, a)[1])[0] < total(build(2, (a + a))) else ((15 ^ a) - 15))
    return (((16 ^ b) - 12) if ap(lambda x: x * 10 + 5, 5) < total(build(1, total(build(3, 2)))) else (6 & total(build(6, a))))


def h1(a, b):
    x = a
    return h0(b, 17)


def h2(a, b):
    x = pair(h0(a, total(build(6, b))), h0((8 - a), h0(11, 5)))[0]
    return (3 if (a if ap(lambda x: x * x + 1, x) < x else total(build(2, 13))) < pair(pair(12, 5)[1], ap(lambda x: x * b + 0, 19))[1] else ap(lambda x: x * (10 if 15 < 11 else 9) + 8, (a if b < 6 else 6)))


def main():
    y = total(build(3, total(build(6, total(build(0, 7))))))
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * y + 0, y) + 7, (y + 6)) + y
    l = build(2, y)
    return (y, ap(lambda x: x * pair(ap(lambda x: x * 16 + 6, y), 3)[0] + 0, ap(lambda x: x * total(build(0, y)) + 8, 16)), f(y) + f(y), total(l) + total(l))
