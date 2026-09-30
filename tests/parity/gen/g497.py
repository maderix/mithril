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
    return (ap(lambda x: x * total(build(6, 3)) + 6, (16 - 14)) if pair(pair(1, 9)[0], ap(lambda x: x * b + 2, a))[1] < 3 else pair((13 & a), ap(lambda x: x * x + 6, a))[1])


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * 7 + 0, ap(lambda x: x * 0 + 1, 3)) + 1, a)
    return total(build(6, total(build(2, h0(b, 4)))))


def h2(a, b):
    x = pair(total(build(3, b)), 10)[0]
    return ap(lambda x: x * total(build(3, ap(lambda x: x * 5 + 7, 8))) + 5, (x if (x * 14) < a else (2 - b)))


def main():
    y = 2
    f = lambda x: x * total(build(1, ap(lambda x: x * 15 + 7, y))) + y
    l = build(1, y)
    return (h1(y, pair(h1(y, y), total(build(2, 17)))[0]), (pair(total(build(2, 17)), ap(lambda x: x * y + 7, y))[1] if (ap(lambda x: x * y + 8, y) ^ ap(lambda x: x * y + 6, 13)) < total(build(3, 10)) else ap(lambda x: x * ap(lambda x: x * y + 1, 9) + 3, total(build(0, y)))), f(h1(y, pair(h1(y, y), total(build(2, 17)))[0])) + f(y), total(l) + total(l))
