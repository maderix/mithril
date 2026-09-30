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
    return (x if 18 < total(build(3, 3)) else (12 + 18))


def h1(a, b):
    x = (total(build(2, a)) if (h0(15, a) if pair(a, 3)[0] < 14 else 11) < 12 else (pair(b, b)[0] if h0(b, 10) < a else total(build(5, 7))))
    return pair(16, ap(lambda x: x * ap(lambda x: x * x + 3, 17) + 3, (a + 18)))[0]


def h2(a, b):
    x = 6
    return (18 if total(build(4, 10)) < (total(build(1, 3)) * (17 if 5 < a else x)) else pair(total(build(5, a)), h0(a, x))[0])


def main():
    y = ((h1(17, 4) if pair(14, 14)[1] < 12 else (2 - 14)) if h1((14 * 9), 10) < 19 else 9)
    f = lambda x: x * y + y
    l = build(1, y)
    return (ap(lambda x: x * ap(lambda x: x * y + 0, ap(lambda x: x * 19 + 2, 14)) + 8, 16), (h2(11, h1(y, y)) ^ y), f(ap(lambda x: x * ap(lambda x: x * y + 0, ap(lambda x: x * 19 + 2, 14)) + 8, 16)) + f(y), total(l) + total(l))
