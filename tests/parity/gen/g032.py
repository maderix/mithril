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
    x = 19
    return 14


def h1(a, b):
    x = 3
    return a


def h2(a, b):
    x = ap(lambda x: x * b + 3, total(build(1, pair(0, 4)[1])))
    return a


def main():
    y = ap(lambda x: x * 0 + 8, pair(10, pair(6, 8)[1])[1])
    f = lambda x: x * 4 + y
    l = build(6, y)
    return (((y & 16) ^ y), total(build(5, (17 if ap(lambda x: x * 18 + 8, 11) < total(build(2, y)) else (y & 19)))), f(((y & 16) ^ y)) + f(y), total(l) + total(l))
