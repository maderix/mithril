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
    x = ap(lambda x: x * 0 + 6, pair(10, (17 if b < a else a))[0])
    return (total(build(2, (b if 12 < a else a))) if pair(x, ap(lambda x: x * 15 + 8, a))[1] < (1 - ap(lambda x: x * 9 + 7, b)) else 16)


def h1(a, b):
    x = 4
    return a


def h2(a, b):
    x = b
    return h0((h1(19, 19) if b < a else pair(2, 2)[0]), 9)


def main():
    y = ap(lambda x: x * h2(ap(lambda x: x * 17 + 2, 3), pair(9, 12)[0]) + 0, total(build(3, total(build(3, 9)))))
    f = lambda x: x * (h2(18, 1) if 19 < ap(lambda x: x * y + 6, 6) else total(build(0, y))) + y
    l = build(3, y)
    return (5, 16, f(5) + f(y), total(l) + total(l))
