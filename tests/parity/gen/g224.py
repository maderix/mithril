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
    x = ap(lambda x: x * (total(build(2, a)) if (17 * 1) < 18 else b) + 3, 5)
    return ((pair(18, a)[0] & a) * 11)


def h1(a, b):
    x = h0(h0((b - b), a), a)
    return ap(lambda x: x * b + 3, ap(lambda x: x * (a if 13 < x else x) + 1, 5))


def h2(a, b):
    x = total(build(0, 9))
    return 4


def main():
    y = pair(17, total(build(1, (15 if 10 < 19 else 9))))[1]
    f = lambda x: x * pair(ap(lambda x: x * y + 5, y), pair(y, y)[0])[0] + y
    l = build(4, y)
    return (pair(6, pair(15, pair(4, 8)[0])[1])[1], h0((pair(4, 5)[1] if (y ^ y) < y else total(build(1, 7))), (ap(lambda x: x * 15 + 8, y) if ap(lambda x: x * y + 1, 1) < ap(lambda x: x * y + 1, y) else y)), f(pair(6, pair(15, pair(4, 8)[0])[1])[1]) + f(y), total(l) + total(l))
