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
    x = 14
    return a


def h1(a, b):
    x = h0(pair(total(build(3, a)), total(build(1, a)))[1], 0)
    return ((x + (3 + 1)) + (h0(10, b) - h0(8, b)))


def h2(a, b):
    x = h1(h1(ap(lambda x: x * 2 + 6, 10), pair(a, 3)[1]), ((b if b < a else a) * a))
    return (pair(total(build(3, 8)), a)[1] * b)


def main():
    y = 8
    f = lambda x: x * ap(lambda x: x * y + 3, pair(y, y)[0]) + y
    l = build(5, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
