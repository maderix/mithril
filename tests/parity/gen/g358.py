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
    x = ap(lambda x: x * total(build(6, ap(lambda x: x * 10 + 5, 19))) + 1, ((19 if b < a else 19) if 7 < ap(lambda x: x * 5 + 6, b) else ap(lambda x: x * a + 3, 0)))
    return pair(pair((a if x < a else b), (x - 2))[1], ap(lambda x: x * 10 + 6, ap(lambda x: x * x + 3, b)))[1]


def h1(a, b):
    x = a
    return pair(total(build(5, ap(lambda x: x * 2 + 8, 7))), total(build(2, pair(b, 8)[1])))[0]


def h2(a, b):
    x = pair(13, total(build(2, b)))[0]
    return (((6 & x) * (x + 3)) - h0(1, 19))


def main():
    y = pair(total(build(2, 16)), ap(lambda x: x * 2 + 0, ap(lambda x: x * 4 + 0, 7)))[1]
    f = lambda x: x * 5 + y
    l = build(2, y)
    return (total(build(6, y)), 7, f(total(build(6, y))) + f(y), total(l) + total(l))
