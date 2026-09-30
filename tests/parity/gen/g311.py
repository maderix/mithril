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
    return x


def h1(a, b):
    x = h0(a, 4)
    return 10


def h2(a, b):
    x = a
    return 15


def main():
    y = pair(total(build(5, (8 * 9))), 7)[0]
    f = lambda x: x * y + y
    l = build(4, y)
    return (h1(total(build(6, total(build(4, y)))), (ap(lambda x: x * 3 + 2, 11) - pair(y, 0)[1])), h2(total(build(5, h0(y, 13))), h2(12, 16)), f(h1(total(build(6, total(build(4, y)))), (ap(lambda x: x * 3 + 2, 11) - pair(y, 0)[1]))) + f(y), total(l) + total(l))
