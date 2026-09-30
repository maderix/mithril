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
    x = total(build(4, total(build(4, 16))))
    return a


def h1(a, b):
    x = (total(build(6, pair(9, 7)[0])) ^ (ap(lambda x: x * 9 + 0, a) if a < total(build(0, a)) else ap(lambda x: x * 16 + 7, 15)))
    return 4


def h2(a, b):
    x = (total(build(1, 12)) if (h1(18, 11) & pair(18, 18)[1]) < pair(0, (8 if 12 < b else 10))[1] else a)
    return 16


def main():
    y = pair((h2(7, 4) + ap(lambda x: x * 16 + 7, 13)), (5 if total(build(1, 11)) < h2(1, 6) else 15))[1]
    f = lambda x: x * h1(h1(y, 9), ap(lambda x: x * 16 + 0, y)) + y
    l = build(3, y)
    return (pair(pair(h1(y, 10), 1)[0], ap(lambda x: x * (y & 14) + 6, (y + y)))[0], ap(lambda x: x * y + 4, (total(build(0, y)) ^ pair(y, y)[1])), f(pair(pair(h1(y, 10), 1)[0], ap(lambda x: x * (y & 14) + 6, (y + y)))[0]) + f(y), total(l) + total(l))
