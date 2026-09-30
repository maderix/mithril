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
    x = pair(total(build(3, pair(9, 19)[1])), pair(ap(lambda x: x * a + 6, b), (a if 10 < 17 else b))[1])[0]
    return 2


def h1(a, b):
    x = h0(total(build(1, (b + b))), ((b ^ 10) if (18 + b) < a else total(build(5, b))))
    return pair(a, pair(a, ap(lambda x: x * x + 5, 17))[0])[1]


def h2(a, b):
    x = (a if 17 < (ap(lambda x: x * 11 + 7, a) if 9 < pair(2, b)[1] else b) else h1(ap(lambda x: x * a + 1, b), (a + 4)))
    return ap(lambda x: x * 15 + 6, 8)


def main():
    y = 12
    f = lambda x: x * y + y
    l = build(2, y)
    return (ap(lambda x: x * ap(lambda x: x * 4 + 6, (y - 15)) + 6, ((y if 14 < 0 else y) if 1 < h2(y, y) else total(build(0, 18)))), ap(lambda x: x * h0(12, h2(18, y)) + 3, total(build(3, 17))), f(ap(lambda x: x * ap(lambda x: x * 4 + 6, (y - 15)) + 6, ((y if 14 < 0 else y) if 1 < h2(y, y) else total(build(0, 18))))) + f(y), total(l) + total(l))
