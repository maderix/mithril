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
    x = 9
    return a


def h1(a, b):
    x = a
    return (total(build(1, (6 if 4 < 4 else b))) if h0(15, pair(6, 9)[1]) < (13 + h0(b, a)) else ap(lambda x: x * pair(a, 10)[0] + 2, ap(lambda x: x * 7 + 3, 6)))


def h2(a, b):
    x = pair(h1(pair(13, 2)[1], total(build(3, a))), ap(lambda x: x * (a if 19 < 1 else 10) + 0, total(build(4, a))))[1]
    return 8


def main():
    y = (pair(h1(8, 13), h0(12, 14))[0] & ((9 * 9) & 10))
    f = lambda x: x * (ap(lambda x: x * y + 0, y) if y < 15 else (y + y)) + y
    l = build(3, y)
    return (11, ap(lambda x: x * pair(ap(lambda x: x * y + 6, y), (6 ^ y))[0] + 5, pair((y ^ 19), 1)[0]), f(11) + f(y), total(l) + total(l))
