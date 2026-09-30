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
    x = 16
    return ap(lambda x: x * ap(lambda x: x * 0 + 4, total(build(2, a))) + 5, total(build(1, ap(lambda x: x * 6 + 5, 18))))


def h1(a, b):
    x = total(build(4, a))
    return pair(((a if x < 8 else 1) - (a if b < x else a)), pair(total(build(0, a)), 11)[1])[1]


def h2(a, b):
    x = h1(0, (h0(16, b) if (14 if b < 4 else b) < pair(8, b)[1] else total(build(5, b))))
    return ap(lambda x: x * b + 4, x)


def main():
    y = pair(h2((5 if 0 < 5 else 19), pair(5, 3)[1]), total(build(0, ap(lambda x: x * 3 + 6, 19))))[1]
    f = lambda x: x * ap(lambda x: x * total(build(2, y)) + 3, h1(y, y)) + y
    l = build(6, y)
    return (pair(total(build(6, ap(lambda x: x * y + 0, 2))), y)[1], h1(pair(pair(y, y)[1], (y ^ 2))[0], ap(lambda x: x * (5 if y < y else 17) + 2, 17)), f(pair(total(build(6, ap(lambda x: x * y + 0, 2))), y)[1]) + f(y), total(l) + total(l))
