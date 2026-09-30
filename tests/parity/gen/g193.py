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
    x = (ap(lambda x: x * ap(lambda x: x * 19 + 3, 16) + 7, a) + 8)
    return (b if (18 * (x if b < 4 else b)) < a else total(build(0, 2)))


def h1(a, b):
    x = b
    return pair(ap(lambda x: x * total(build(2, 8)) + 6, (a if 4 < 4 else 19)), ap(lambda x: x * pair(1, 4)[1] + 1, (x if b < 3 else 7)))[1]


def h2(a, b):
    x = ap(lambda x: x * (11 & a) + 2, h1(h1(b, a), ap(lambda x: x * a + 2, 6)))
    return h1(pair((a if b < 4 else x), ap(lambda x: x * x + 3, b))[1], x)


def main():
    y = ap(lambda x: x * ((5 if 7 < 19 else 11) + 12) + 5, 11)
    f = lambda x: x * 18 + y
    l = build(6, y)
    return (y, ap(lambda x: x * 15 + 3, 18), f(y) + f(y), total(l) + total(l))
