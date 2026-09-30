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
    x = 17
    return ap(lambda x: x * ap(lambda x: x * 3 + 8, total(build(3, a))) + 5, total(build(3, (a if 19 < 7 else b))))


def h1(a, b):
    x = (19 - total(build(6, (11 + b))))
    return (pair((7 if 19 < 8 else 6), total(build(3, 17)))[1] ^ 17)


def h2(a, b):
    x = total(build(1, ap(lambda x: x * total(build(2, 2)) + 6, total(build(0, b)))))
    return (total(build(6, b)) + a)


def main():
    y = ap(lambda x: x * h1(pair(4, 4)[1], total(build(3, 4))) + 3, total(build(4, total(build(5, 18)))))
    f = lambda x: x * ap(lambda x: x * h0(18, 13) + 2, pair(16, 12)[0]) + y
    l = build(2, y)
    return (ap(lambda x: x * y + 0, y), (y * y), f(ap(lambda x: x * y + 0, y)) + f(y), total(l) + total(l))
