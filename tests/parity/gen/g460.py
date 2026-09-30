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
    x = total(build(5, ((b ^ 13) if b < (0 ^ b) else 15)))
    return total(build(2, ap(lambda x: x * 9 + 6, 5)))


def h1(a, b):
    x = total(build(3, 12))
    return pair(pair(ap(lambda x: x * b + 2, b), (10 if 17 < 0 else 9))[1], 1)[1]


def h2(a, b):
    x = total(build(0, (total(build(6, b)) + b)))
    return total(build(4, pair(ap(lambda x: x * x + 2, b), pair(x, 15)[1])[0]))


def main():
    y = pair(12, h0((12 if 17 < 6 else 12), (2 if 19 < 3 else 0)))[1]
    f = lambda x: x * ((18 * 15) if (y - 3) < y else 0) + y
    l = build(2, y)
    return ((h0(13, pair(9, y)[1]) if h1(4, ap(lambda x: x * 19 + 4, y)) < pair(h2(5, 6), y)[1] else ((y - 0) if (y - y) < (19 ^ 18) else total(build(0, 8)))), h1(total(build(4, h1(11, y))), (4 if y < h0(y, 16) else 12)), f((h0(13, pair(9, y)[1]) if h1(4, ap(lambda x: x * 19 + 4, y)) < pair(h2(5, 6), y)[1] else ((y - 0) if (y - y) < (19 ^ 18) else total(build(0, 8))))) + f(y), total(l) + total(l))
