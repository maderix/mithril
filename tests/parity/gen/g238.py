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
    x = 1
    return 7


def h1(a, b):
    x = 2
    return a


def h2(a, b):
    x = h0((8 if a < 6 else h1(b, 4)), total(build(2, ap(lambda x: x * a + 5, 2))))
    return a


def main():
    y = ap(lambda x: x * pair(16, h2(15, 15))[0] + 6, 2)
    f = lambda x: x * h2(h1(19, y), total(build(6, y))) + y
    l = build(2, y)
    return (1, y, f(1) + f(y), total(l) + total(l))
