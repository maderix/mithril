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
    x = ((pair(a, b)[0] if b < a else (b if 13 < 19 else 12)) if 6 < total(build(0, total(build(1, a)))) else ap(lambda x: x * total(build(6, b)) + 7, ap(lambda x: x * a + 3, 1)))
    return 19


def h1(a, b):
    x = (h0((a if 19 < 5 else b), (15 * a)) if 13 < 3 else h0((b if 9 < a else b), 4))
    return 5


def h2(a, b):
    x = ap(lambda x: x * a + 5, pair(total(build(2, a)), (19 - a))[0])
    return ap(lambda x: x * total(build(1, 5)) + 3, ap(lambda x: x * a + 2, a))


def main():
    y = pair(7, ap(lambda x: x * 4 + 4, (4 if 13 < 2 else 2)))[0]
    f = lambda x: x * (0 if h1(16, y) < total(build(4, y)) else pair(4, y)[1]) + y
    l = build(1, y)
    return (total(build(0, h0(h2(y, y), ap(lambda x: x * y + 3, 4)))), h1(((10 if 0 < 8 else 12) if (y + 3) < h1(10, 8) else ap(lambda x: x * y + 5, 0)), pair(pair(17, 11)[1], (15 ^ 6))[0]), f(total(build(0, h0(h2(y, y), ap(lambda x: x * y + 3, 4))))) + f(y), total(l) + total(l))
