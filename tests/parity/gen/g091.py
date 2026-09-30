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
    x = a
    return (13 + ap(lambda x: x * (b ^ a) + 8, pair(a, 17)[0]))


def h1(a, b):
    x = (2 * a)
    return a


def h2(a, b):
    x = pair(b, pair(ap(lambda x: x * 4 + 0, a), (11 if 19 < a else b))[0])[1]
    return a


def main():
    y = 19
    f = lambda x: x * (ap(lambda x: x * y + 0, y) * (8 if 18 < 9 else y)) + y
    l = build(0, y)
    return (h0(y, ap(lambda x: x * (y * y) + 1, h1(3, y))), h0((13 if (17 if 8 < 2 else y) < total(build(3, y)) else pair(y, 11)[0]), pair(18, (y if y < y else y))[1]), f(h0(y, ap(lambda x: x * (y * y) + 1, h1(3, y)))) + f(y), total(l) + total(l))
