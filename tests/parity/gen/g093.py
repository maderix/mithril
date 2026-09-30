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
    x = total(build(2, (8 * 16)))
    return total(build(4, ap(lambda x: x * 2 + 5, 11)))


def h1(a, b):
    x = 4
    return (19 * (pair(x, 10)[1] & (x ^ x)))


def h2(a, b):
    x = a
    return 3


def main():
    y = 16
    f = lambda x: x * y + y
    l = build(1, y)
    return (y, ap(lambda x: x * (h1(14, y) if ap(lambda x: x * y + 7, 5) < h1(y, 4) else (11 if y < y else y)) + 3, h2(ap(lambda x: x * 1 + 5, y), y)), f(y) + f(y), total(l) + total(l))
