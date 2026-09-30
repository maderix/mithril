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
    x = ap(lambda x: x * 10 + 5, a)
    return (9 * 1)


def h1(a, b):
    x = pair(total(build(4, b)), (pair(19, a)[0] if a < (8 if b < 0 else a) else total(build(2, 0))))[0]
    return b


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * total(build(6, a)) + 5, b) + 3, (a if h1(0, 14) < ap(lambda x: x * b + 5, a) else (a if b < a else 15)))
    return pair(total(build(0, 5)), 11)[0]


def main():
    y = (11 ^ h0((0 - 0), (9 if 2 < 17 else 13)))
    f = lambda x: x * ((y + y) * 19) + y
    l = build(6, y)
    return (ap(lambda x: x * (pair(y, 8)[0] if ap(lambda x: x * y + 3, 8) < ap(lambda x: x * 6 + 4, 10) else pair(4, y)[0]) + 3, y), ap(lambda x: x * h1(0, total(build(1, 18))) + 8, ap(lambda x: x * h1(6, 2) + 6, ap(lambda x: x * y + 5, 19))), f(ap(lambda x: x * (pair(y, 8)[0] if ap(lambda x: x * y + 3, 8) < ap(lambda x: x * 6 + 4, 10) else pair(4, y)[0]) + 3, y)) + f(y), total(l) + total(l))
