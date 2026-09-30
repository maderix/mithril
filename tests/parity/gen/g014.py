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
    x = ap(lambda x: x * pair(total(build(0, 6)), 14)[1] + 8, ap(lambda x: x * total(build(5, a)) + 5, (b if b < a else 0)))
    return pair(total(build(1, ap(lambda x: x * x + 3, 18))), ap(lambda x: x * total(build(2, b)) + 6, 1))[0]


def h1(a, b):
    x = h0(13, h0(h0(16, a), pair(b, b)[1]))
    return h0(total(build(3, 11)), pair(pair(11, 10)[1], (9 if 9 < 0 else 2))[1])


def h2(a, b):
    x = a
    return 8


def main():
    y = (ap(lambda x: x * 1 + 1, (10 if 11 < 6 else 17)) if pair(h1(10, 17), h0(4, 19))[1] < ((18 - 1) if ap(lambda x: x * 7 + 2, 17) < ap(lambda x: x * 2 + 7, 5) else total(build(6, 18))) else 5)
    f = lambda x: x * y + y
    l = build(5, y)
    return (h2(total(build(5, h0(y, 14))), pair(h1(7, 0), (y if 3 < 13 else y))[0]), (pair(total(build(3, 8)), 2)[0] + total(build(3, 13))), f(h2(total(build(5, h0(y, 14))), pair(h1(7, 0), (y if 3 < 13 else y))[0])) + f(y), total(l) + total(l))
