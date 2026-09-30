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
    x = pair(13, b)[0]
    return 17


def h1(a, b):
    x = 4
    return (ap(lambda x: x * (13 - b) + 6, h0(10, b)) if 4 < ap(lambda x: x * b + 3, x) else x)


def h2(a, b):
    x = total(build(5, (h1(4, 19) if ap(lambda x: x * a + 3, 3) < pair(b, 12)[0] else total(build(4, b)))))
    return h0(((19 if 5 < x else b) if h0(8, 1) < 18 else x), h1(x, ap(lambda x: x * x + 3, a)))


def main():
    y = ap(lambda x: x * total(build(4, ap(lambda x: x * 12 + 1, 13))) + 1, pair(h1(0, 18), 19)[1])
    f = lambda x: x * 19 + y
    l = build(3, y)
    return (ap(lambda x: x * (h0(2, y) if (8 if y < y else y) < ap(lambda x: x * 7 + 3, 18) else total(build(2, 8))) + 2, total(build(1, (y if y < 15 else 2)))), (ap(lambda x: x * 12 + 8, (y & y)) if 13 < h1(ap(lambda x: x * y + 5, 14), h2(18, 4)) else total(build(5, pair(y, y)[1]))), f(ap(lambda x: x * (h0(2, y) if (8 if y < y else y) < ap(lambda x: x * 7 + 3, 18) else total(build(2, 8))) + 2, total(build(1, (y if y < 15 else 2))))) + f(y), total(l) + total(l))
