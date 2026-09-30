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
    x = (5 if (pair(b, b)[0] + 3) < ap(lambda x: x * 14 + 2, total(build(2, 7))) else ap(lambda x: x * pair(a, a)[1] + 6, a))
    return (pair(0, ap(lambda x: x * 17 + 5, 16))[1] if pair(4, (5 if 4 < a else x))[1] < 16 else ((x & b) if pair(5, 3)[0] < 5 else ap(lambda x: x * 2 + 8, 19)))


def h1(a, b):
    x = 17
    return h0(a, 15)


def h2(a, b):
    x = ap(lambda x: x * pair(ap(lambda x: x * 17 + 2, 5), (8 + b))[1] + 5, ap(lambda x: x * total(build(6, 19)) + 8, 3))
    return 13


def main():
    y = total(build(6, ap(lambda x: x * (0 if 19 < 5 else 13) + 0, total(build(4, 5)))))
    f = lambda x: x * ap(lambda x: x * total(build(4, 11)) + 8, total(build(4, y))) + y
    l = build(6, y)
    return ((11 - h1(ap(lambda x: x * 9 + 3, 4), y)), 7, f((11 - h1(ap(lambda x: x * 9 + 3, 4), y))) + f(y), total(l) + total(l))
