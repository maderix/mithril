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
    x = 7
    return ap(lambda x: x * pair(0, a)[1] + 6, 13)


def h1(a, b):
    x = (pair(h0(3, a), (5 * 19))[0] if 0 < pair(h0(18, 6), total(build(6, 6)))[0] else h0(ap(lambda x: x * a + 1, a), total(build(6, b))))
    return h0(a, (9 - (12 if b < 3 else b)))


def h2(a, b):
    x = pair(0, h1((b if 11 < b else 10), total(build(0, 15))))[0]
    return ap(lambda x: x * total(build(0, ap(lambda x: x * 5 + 6, 0))) + 0, h1((11 ^ a), (x if 4 < b else b)))


def main():
    y = (total(build(3, 19)) + h2(total(build(1, 7)), (2 if 19 < 0 else 15)))
    f = lambda x: x * h1(15, (5 - 5)) + y
    l = build(0, y)
    return (ap(lambda x: x * ap(lambda x: x * (3 ^ y) + 8, h0(4, y)) + 0, y), pair(ap(lambda x: x * pair(y, y)[1] + 7, y), (7 if 15 < y else pair(y, 5)[0]))[0], f(ap(lambda x: x * ap(lambda x: x * (3 ^ y) + 8, h0(4, y)) + 0, y)) + f(y), total(l) + total(l))
