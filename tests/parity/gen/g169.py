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
    x = total(build(0, 14))
    return pair(9, (pair(x, 12)[1] - pair(b, 15)[0]))[1]


def h1(a, b):
    x = ap(lambda x: x * b + 2, ap(lambda x: x * b + 6, pair(a, 9)[1]))
    return a


def h2(a, b):
    x = 16
    return (h0(a, (a if 7 < 2 else 10)) * ((a if 6 < 14 else 11) & pair(9, 1)[1]))


def main():
    y = 14
    f = lambda x: x * pair(pair(y, 7)[0], y)[0] + y
    l = build(2, y)
    return (total(build(2, (pair(y, 4)[0] + y))), 15, f(total(build(2, (pair(y, 4)[0] + y)))) + f(y), total(l) + total(l))
