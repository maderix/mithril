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
    x = (0 if 11 < (ap(lambda x: x * a + 5, 11) ^ (9 if a < b else 6)) else ap(lambda x: x * 13 + 7, 4))
    return a


def h1(a, b):
    x = h0((h0(b, b) if total(build(3, 10)) < (a ^ 16) else (b if 19 < 6 else 19)), (h0(a, b) if (b * b) < 10 else pair(10, b)[0]))
    return 17


def h2(a, b):
    x = (pair(a, total(build(0, 13)))[0] if total(build(1, pair(6, 2)[1])) < 15 else a)
    return 13


def main():
    y = h0(total(build(3, 9)), pair(pair(12, 13)[1], 10)[1])
    f = lambda x: x * (y if total(build(3, 12)) < pair(y, 2)[0] else h1(y, 1)) + y
    l = build(6, y)
    return (h2(18, y), (10 if h0(h1(y, 0), ap(lambda x: x * y + 6, 19)) < (y if pair(y, y)[0] < h0(y, y) else y) else total(build(6, ap(lambda x: x * 11 + 4, 13)))), f(h2(18, y)) + f(y), total(l) + total(l))
