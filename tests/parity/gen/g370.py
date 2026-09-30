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
    x = pair((pair(b, b)[1] if 9 < 13 else (18 if 6 < 5 else 1)), (17 * ap(lambda x: x * 7 + 4, b)))[0]
    return 4


def h1(a, b):
    x = ap(lambda x: x * 9 + 4, h0(h0(b, 4), h0(8, 1)))
    return ap(lambda x: x * 2 + 7, h0(b, pair(x, b)[0]))


def h2(a, b):
    x = (h1(pair(a, 12)[0], a) ^ (total(build(6, b)) + total(build(6, 3))))
    return h0(ap(lambda x: x * x + 5, h0(14, 14)), pair(pair(4, 7)[1], h1(9, x))[1])


def main():
    y = (pair(pair(12, 2)[1], total(build(6, 15)))[0] if 17 < ((11 if 10 < 15 else 13) - 5) else total(build(0, (6 if 7 < 17 else 9))))
    f = lambda x: x * 2 + y
    l = build(1, y)
    return (13, 14, f(13) + f(y), total(l) + total(l))
