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
    x = (4 if ((14 * 13) if 9 < (17 - a) else pair(16, b)[1]) < b else ap(lambda x: x * a + 6, b))
    return ap(lambda x: x * total(build(3, pair(b, 18)[1])) + 8, x)


def h1(a, b):
    x = a
    return 5


def h2(a, b):
    x = ap(lambda x: x * (a + h1(17, 0)) + 3, ap(lambda x: x * (a ^ b) + 1, b))
    return h1((h0(x, 17) if h1(18, x) < h1(11, 14) else pair(x, 11)[1]), total(build(6, 5)))


def main():
    y = 15
    f = lambda x: x * ap(lambda x: x * y + 7, ap(lambda x: x * 11 + 1, 19)) + y
    l = build(1, y)
    return (y, 16, f(y) + f(y), total(l) + total(l))
