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
    x = total(build(0, total(build(2, pair(0, b)[1]))))
    return pair(18, ap(lambda x: x * 2 + 1, 16))[1]


def h1(a, b):
    x = total(build(5, h0(ap(lambda x: x * 15 + 4, b), (12 if a < a else b))))
    return b


def h2(a, b):
    x = total(build(6, (pair(b, b)[1] * b)))
    return ap(lambda x: x * h0((17 if 16 < 8 else b), h1(b, x)) + 4, pair(6, ap(lambda x: x * 16 + 7, 5))[1])


def main():
    y = h0(10, 6)
    f = lambda x: x * y + y
    l = build(5, y)
    return (h0(pair(total(build(5, y)), 4)[1], (ap(lambda x: x * 7 + 7, y) ^ pair(y, y)[1])), (y & ap(lambda x: x * total(build(1, y)) + 5, ap(lambda x: x * y + 6, 19))), f(h0(pair(total(build(5, y)), 4)[1], (ap(lambda x: x * 7 + 7, y) ^ pair(y, y)[1]))) + f(y), total(l) + total(l))
