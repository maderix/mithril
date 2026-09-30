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
    x = 4
    return 10


def h1(a, b):
    x = 11
    return (b if h0(pair(0, 1)[0], total(build(6, 8))) < (total(build(6, a)) if ap(lambda x: x * b + 5, 6) < total(build(6, x)) else x) else x)


def h2(a, b):
    x = 19
    return a


def main():
    y = 16
    f = lambda x: x * ap(lambda x: x * (3 if 14 < y else y) + 6, pair(11, y)[0]) + y
    l = build(5, y)
    return ((((10 if 18 < 9 else y) * total(build(1, y))) if 10 < 0 else total(build(6, (y & 19)))), h2((ap(lambda x: x * 15 + 7, 14) - 14), ((y & 10) if total(build(5, y)) < (8 & 2) else ap(lambda x: x * y + 6, y))), f((((10 if 18 < 9 else y) * total(build(1, y))) if 10 < 0 else total(build(6, (y & 19))))) + f(y), total(l) + total(l))
