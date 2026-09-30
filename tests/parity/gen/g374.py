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
    x = (a & 13)
    return 9


def h1(a, b):
    x = ap(lambda x: x * (pair(a, a)[0] + a) + 0, h0((11 ^ a), 3))
    return a


def h2(a, b):
    x = 2
    return total(build(4, ap(lambda x: x * pair(x, b)[0] + 3, h1(b, 14))))


def main():
    y = 10
    f = lambda x: x * h2(y, y) + y
    l = build(2, y)
    return (0, y, f(0) + f(y), total(l) + total(l))
