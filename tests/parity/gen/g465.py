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
    x = a
    return total(build(2, total(build(2, b))))


def h1(a, b):
    x = ap(lambda x: x * 17 + 2, a)
    return a


def h2(a, b):
    x = a
    return ap(lambda x: x * b + 0, h0((14 * 19), ap(lambda x: x * 15 + 4, 14)))


def main():
    y = 8
    f = lambda x: x * h2(ap(lambda x: x * 5 + 7, y), y) + y
    l = build(6, y)
    return (14, y, f(14) + f(y), total(l) + total(l))
