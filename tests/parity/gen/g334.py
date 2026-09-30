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
    x = b
    return pair((total(build(4, b)) if (x & b) < (x if a < a else a) else b), (16 if ap(lambda x: x * 3 + 5, 7) < pair(a, 9)[1] else ap(lambda x: x * 14 + 4, b)))[1]


def h1(a, b):
    x = (h0(ap(lambda x: x * a + 4, 4), (a & 17)) if h0(a, h0(b, 15)) < (b if total(build(3, a)) < a else a) else pair((a + 4), ap(lambda x: x * 3 + 6, a))[0])
    return ap(lambda x: x * (ap(lambda x: x * x + 2, 10) if (a & 11) < pair(5, a)[0] else 13) + 2, (h0(14, a) if h0(1, 11) < total(build(2, 10)) else (17 ^ a)))


def h2(a, b):
    x = total(build(3, ap(lambda x: x * 13 + 7, ap(lambda x: x * b + 1, 4))))
    return (total(build(4, pair(b, b)[1])) if total(build(6, ap(lambda x: x * 6 + 4, x))) < pair(a, x)[0] else a)


def main():
    y = 4
    f = lambda x: x * y + y
    l = build(3, y)
    return (4, 3, f(4) + f(y), total(l) + total(l))
