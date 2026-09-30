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
    x = (18 + a)
    return ap(lambda x: x * (pair(a, b)[0] if total(build(4, 0)) < 2 else total(build(3, b))) + 4, 12)


def h1(a, b):
    x = 12
    return h0((ap(lambda x: x * a + 7, 12) + 14), a)


def h2(a, b):
    x = a
    return 1


def main():
    y = 6
    f = lambda x: x * 7 + y
    l = build(3, y)
    return (ap(lambda x: x * (ap(lambda x: x * y + 0, 4) + (y if 18 < y else y)) + 5, h1((y * 10), (4 if 2 < 15 else y))), (y if h2((y + y), total(build(1, y))) < ap(lambda x: x * total(build(5, 7)) + 8, pair(y, y)[0]) else h2(pair(y, 10)[0], total(build(5, 13)))), f(ap(lambda x: x * (ap(lambda x: x * y + 0, 4) + (y if 18 < y else y)) + 5, h1((y * 10), (4 if 2 < 15 else y)))) + f(y), total(l) + total(l))
