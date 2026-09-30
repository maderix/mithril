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
    x = pair(a, (3 + (a if 4 < 0 else a)))[0]
    return x


def h1(a, b):
    x = total(build(2, (a if a < total(build(5, 7)) else h0(b, a))))
    return pair(h0(pair(a, 17)[1], (18 if b < a else b)), 13)[1]


def h2(a, b):
    x = total(build(0, pair(ap(lambda x: x * 9 + 5, 7), a)[1]))
    return h0(7, b)


def main():
    y = (13 & h1((7 + 18), 17))
    f = lambda x: x * (h0(18, y) ^ (13 if 18 < y else 8)) + y
    l = build(5, y)
    return (pair(ap(lambda x: x * pair(y, 12)[1] + 3, y), (h0(11, y) if h2(8, 10) < ap(lambda x: x * y + 7, y) else h2(y, 7)))[1], (((y + y) & total(build(5, 14))) + 19), f(pair(ap(lambda x: x * pair(y, 12)[1] + 3, y), (h0(11, y) if h2(8, 10) < ap(lambda x: x * y + 7, y) else h2(y, 7)))[1]) + f(y), total(l) + total(l))
