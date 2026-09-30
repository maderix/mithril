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
    return 5


def h1(a, b):
    x = h0((11 & pair(a, 0)[1]), total(build(4, ap(lambda x: x * 8 + 1, 12))))
    return ((pair(6, 14)[1] if ap(lambda x: x * b + 6, 13) < (b * 7) else (7 + a)) if h0(h0(2, b), (15 - 4)) < pair((a + 4), h0(4, 5))[0] else x)


def h2(a, b):
    x = total(build(2, total(build(0, (a ^ a)))))
    return ap(lambda x: x * 18 + 1, total(build(2, (9 + a))))


def main():
    y = (9 ^ (ap(lambda x: x * 19 + 2, 12) * 13))
    f = lambda x: x * 0 + y
    l = build(0, y)
    return (total(build(1, 4)), ap(lambda x: x * total(build(1, ap(lambda x: x * 14 + 0, 19))) + 2, pair(total(build(3, 19)), (8 if 4 < y else 2))[1]), f(total(build(1, 4))) + f(y), total(l) + total(l))
