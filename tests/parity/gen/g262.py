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
    x = ap(lambda x: x * (pair(15, b)[0] if 11 < (19 if b < b else b) else 7) + 0, 0)
    return x


def h1(a, b):
    x = pair(h0(a, (3 if 9 < 15 else a)), 7)[1]
    return a


def h2(a, b):
    x = total(build(5, 7))
    return ((ap(lambda x: x * 8 + 8, 19) ^ h1(x, a)) if (a - pair(6, 3)[1]) < a else 0)


def main():
    y = 3
    f = lambda x: x * total(build(5, (y if 16 < 17 else 4))) + y
    l = build(6, y)
    return (ap(lambda x: x * (total(build(2, y)) if h0(y, 15) < 16 else pair(y, 7)[0]) + 6, total(build(0, ap(lambda x: x * 11 + 7, y)))), ap(lambda x: x * h2(pair(y, y)[0], y) + 8, total(build(6, pair(y, y)[1]))), f(ap(lambda x: x * (total(build(2, y)) if h0(y, 15) < 16 else pair(y, 7)[0]) + 6, total(build(0, ap(lambda x: x * 11 + 7, y))))) + f(y), total(l) + total(l))
