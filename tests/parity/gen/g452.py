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
    x = pair((pair(a, 10)[1] if total(build(3, a)) < ap(lambda x: x * a + 6, 6) else b), b)[0]
    return 7


def h1(a, b):
    x = ap(lambda x: x * (pair(b, a)[0] if 4 < (b if 3 < a else b) else (8 if 4 < b else 0)) + 4, ap(lambda x: x * total(build(5, b)) + 3, pair(13, b)[0]))
    return ((h0(x, 7) if pair(10, a)[1] < (4 - a) else 0) ^ h0(ap(lambda x: x * a + 2, a), h0(b, x)))


def h2(a, b):
    x = total(build(0, b))
    return ((h1(8, 8) if pair(b, x)[1] < h1(10, 7) else (a if a < b else b)) if x < ((b - 18) & ap(lambda x: x * 8 + 8, x)) else total(build(1, ap(lambda x: x * 14 + 4, x))))


def main():
    y = 3
    f = lambda x: x * (pair(1, y)[0] & h1(y, 6)) + y
    l = build(6, y)
    return (h2(14, total(build(6, ap(lambda x: x * 10 + 0, y)))), ap(lambda x: x * (ap(lambda x: x * 19 + 8, y) * ap(lambda x: x * y + 0, 6)) + 7, 4), f(h2(14, total(build(6, ap(lambda x: x * 10 + 0, y))))) + f(y), total(l) + total(l))
