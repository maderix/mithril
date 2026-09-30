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
    x = total(build(6, (ap(lambda x: x * b + 6, 0) * 10)))
    return ap(lambda x: x * 11 + 4, 0)


def h1(a, b):
    x = h0(h0((a - 13), h0(b, 7)), 18)
    return x


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * (4 * a) + 5, pair(0, a)[0]) + 7, h0(pair(11, a)[0], total(build(2, a))))
    return ap(lambda x: x * ((6 if b < x else 9) + 0) + 0, (b & a))


def main():
    y = 15
    f = lambda x: x * 7 + y
    l = build(4, y)
    return (y, total(build(1, 10)), f(y) + f(y), total(l) + total(l))
