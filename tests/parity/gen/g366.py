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
    x = (b if (ap(lambda x: x * 7 + 8, 7) - 0) < 16 else pair(ap(lambda x: x * 2 + 6, a), 1)[0])
    return a


def h1(a, b):
    x = pair(pair(h0(8, 19), h0(10, 1))[0], ((12 ^ b) if h0(16, a) < h0(a, 8) else h0(2, 12)))[1]
    return 6


def h2(a, b):
    x = (total(build(2, total(build(6, a)))) if total(build(2, total(build(5, a)))) < h1(9, pair(17, 14)[1]) else ((3 if a < 11 else b) - pair(b, 6)[0]))
    return h1((ap(lambda x: x * 9 + 0, a) if pair(10, x)[0] < total(build(3, 6)) else (a * 9)), 5)


def main():
    y = 16
    f = lambda x: x * h2(pair(y, 12)[1], (18 & 13)) + y
    l = build(5, y)
    return (((9 if y < (y - 16) else (4 if y < 7 else y)) if pair(18, ap(lambda x: x * y + 4, 2))[0] < (h1(15, 2) if y < 5 else pair(19, 19)[0]) else ((y + 18) if total(build(4, y)) < (12 + y) else (y & y))), h2(ap(lambda x: x * ap(lambda x: x * 15 + 1, y) + 5, ap(lambda x: x * y + 0, y)), 17), f(((9 if y < (y - 16) else (4 if y < 7 else y)) if pair(18, ap(lambda x: x * y + 4, 2))[0] < (h1(15, 2) if y < 5 else pair(19, 19)[0]) else ((y + 18) if total(build(4, y)) < (12 + y) else (y & y)))) + f(y), total(l) + total(l))
