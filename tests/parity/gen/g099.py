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
    x = pair(pair(total(build(1, 5)), total(build(4, b)))[0], ap(lambda x: x * (8 ^ 12) + 7, total(build(4, 1))))[1]
    return (a + ap(lambda x: x * 13 + 8, pair(1, 7)[1]))


def h1(a, b):
    x = (b & a)
    return h0(ap(lambda x: x * (a * b) + 2, h0(7, b)), x)


def h2(a, b):
    x = pair((total(build(1, b)) if h1(a, a) < pair(14, 19)[1] else a), pair(19, h0(16, 17))[0])[1]
    return b


def main():
    y = ap(lambda x: x * pair(13, (19 if 3 < 13 else 11))[0] + 3, h1(h0(3, 4), 4))
    f = lambda x: x * y + y
    l = build(2, y)
    return (y, h0(total(build(5, (y * 3))), y), f(y) + f(y), total(l) + total(l))
