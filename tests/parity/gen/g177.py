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
    x = 8
    return 15


def h1(a, b):
    x = a
    return total(build(2, x))


def h2(a, b):
    x = b
    return 4


def main():
    y = h2(total(build(4, ap(lambda x: x * 11 + 7, 3))), ap(lambda x: x * (6 if 16 < 10 else 15) + 8, (14 if 12 < 3 else 9)))
    f = lambda x: x * pair(total(build(0, 2)), h0(y, y))[1] + y
    l = build(0, y)
    return (h0((total(build(2, y)) if ap(lambda x: x * y + 7, y) < y else 2), y), h2(h2(ap(lambda x: x * y + 3, 1), 13), pair((19 if y < y else y), h2(1, 0))[0]), f(h0((total(build(2, y)) if ap(lambda x: x * y + 7, y) < y else 2), y)) + f(y), total(l) + total(l))
