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
    x = pair(pair(15, 11)[1], a)[0]
    return (a if total(build(0, 12)) < total(build(3, 15)) else total(build(1, x)))


def h1(a, b):
    x = ap(lambda x: x * a + 6, 15)
    return (8 ^ total(build(3, a)))


def h2(a, b):
    x = (((7 if b < 17 else b) if pair(3, a)[1] < (4 * 14) else (9 & b)) + h1(h0(6, b), (18 & b)))
    return total(build(4, h1(h1(2, b), ap(lambda x: x * 11 + 0, a))))


def main():
    y = (17 & 7)
    f = lambda x: x * total(build(5, pair(3, y)[1])) + y
    l = build(2, y)
    return (h1(pair(h1(0, y), ap(lambda x: x * 12 + 7, y))[0], h0(total(build(3, 8)), ap(lambda x: x * 2 + 3, 6))), 16, f(h1(pair(h1(0, y), ap(lambda x: x * 12 + 7, y))[0], h0(total(build(3, 8)), ap(lambda x: x * 2 + 3, 6)))) + f(y), total(l) + total(l))
