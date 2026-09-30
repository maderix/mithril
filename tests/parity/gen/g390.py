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
    x = 4
    return pair(15, ap(lambda x: x * (b + 10) + 7, x))[0]


def h1(a, b):
    x = 18
    return x


def h2(a, b):
    x = h0(h1(4, ap(lambda x: x * a + 6, 12)), ap(lambda x: x * 2 + 7, 15))
    return pair(pair(x, (9 & x))[0], 12)[0]


def main():
    y = 17
    f = lambda x: x * ap(lambda x: x * 3 + 6, (y + y)) + y
    l = build(4, y)
    return (ap(lambda x: x * h0(h0(y, y), pair(y, y)[1]) + 2, ap(lambda x: x * (14 if y < y else y) + 0, y)), total(build(4, ((6 & y) ^ pair(8, y)[1]))), f(ap(lambda x: x * h0(h0(y, y), pair(y, y)[1]) + 2, ap(lambda x: x * (14 if y < y else y) + 0, y))) + f(y), total(l) + total(l))
