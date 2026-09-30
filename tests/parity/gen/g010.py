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
    x = (total(build(5, 17)) + ap(lambda x: x * (a if 2 < b else a) + 6, 9))
    return (total(build(3, 13)) if ap(lambda x: x * 4 + 1, a) < (a if x < 0 else pair(b, b)[1]) else ((7 if x < 2 else 12) ^ (b if 8 < b else a)))


def h1(a, b):
    x = (ap(lambda x: x * total(build(2, b)) + 0, 0) if total(build(0, (3 + 17))) < ((11 & b) if total(build(6, a)) < 5 else h0(18, a)) else ((15 - 11) * (a * 16)))
    return pair(x, total(build(4, 19)))[1]


def h2(a, b):
    x = (total(build(1, total(build(1, a)))) if b < ap(lambda x: x * 6 + 4, total(build(3, b))) else h0(ap(lambda x: x * 15 + 5, 11), (a * b)))
    return pair(pair((b if b < 5 else b), h1(a, 10))[1], pair(total(build(3, 16)), (17 * a))[0])[1]


def main():
    y = ap(lambda x: x * 1 + 4, 3)
    f = lambda x: x * (y & (0 - y)) + y
    l = build(3, y)
    return (y, (h2(pair(y, 19)[0], ap(lambda x: x * y + 5, y)) if h1(h1(y, y), pair(19, 9)[1]) < total(build(4, 19)) else 8), f(y) + f(y), total(l) + total(l))
