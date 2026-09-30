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
    x = ((pair(5, 18)[1] if b < (a + 17) else ap(lambda x: x * 9 + 3, a)) if 5 < ap(lambda x: x * 10 + 5, total(build(0, a))) else b)
    return (17 - (pair(b, b)[0] if 8 < 0 else pair(a, 11)[1]))


def h1(a, b):
    x = h0(h0(pair(a, a)[0], (18 if a < 12 else b)), a)
    return total(build(3, pair(x, ap(lambda x: x * 2 + 8, 16))[1]))


def h2(a, b):
    x = ap(lambda x: x * (ap(lambda x: x * 15 + 6, 12) & 8) + 5, ((a if 15 < 10 else b) if (b & 15) < h1(b, 4) else (16 - a)))
    return x


def main():
    y = (6 & (0 if pair(18, 1)[1] < 5 else (19 if 17 < 1 else 7)))
    f = lambda x: x * ap(lambda x: x * (y ^ 11) + 4, h1(0, y)) + y
    l = build(2, y)
    return (ap(lambda x: x * 3 + 2, pair(y, 6)[1]), h0(h0((15 if 0 < 9 else 18), pair(y, y)[0]), ((y & y) if 0 < total(build(0, 16)) else pair(18, 19)[0])), f(ap(lambda x: x * 3 + 2, pair(y, 6)[1])) + f(y), total(l) + total(l))
