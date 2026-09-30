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
    x = 13
    return pair(pair(ap(lambda x: x * 3 + 4, 9), 6)[1], ((a * b) + ap(lambda x: x * b + 8, b)))[1]


def h1(a, b):
    x = ap(lambda x: x * b + 4, pair((b if 19 < a else b), 1)[0])
    return 16


def h2(a, b):
    x = pair(h1(a, h0(15, b)), ap(lambda x: x * (a if a < 4 else 1) + 1, ap(lambda x: x * a + 0, b)))[0]
    return (h0(pair(13, x)[1], (a + 19)) + 16)


def main():
    y = ((4 & 17) if h0(6, h2(15, 0)) < ((7 + 13) if (15 if 7 < 19 else 18) < pair(15, 7)[0] else 5) else (h0(8, 9) * (17 if 11 < 11 else 7)))
    f = lambda x: x * 17 + y
    l = build(5, y)
    return (total(build(4, ((y ^ y) ^ total(build(2, y))))), pair(pair(y, total(build(6, y)))[0], 15)[1], f(total(build(4, ((y ^ y) ^ total(build(2, y)))))) + f(y), total(l) + total(l))
