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
    x = b
    return total(build(1, total(build(5, 0))))


def h1(a, b):
    x = h0(ap(lambda x: x * 0 + 7, pair(a, 13)[1]), h0(h0(b, a), pair(6, 3)[1]))
    return h0(6, ap(lambda x: x * (a & 11) + 7, pair(19, b)[0]))


def h2(a, b):
    x = ap(lambda x: x * pair(h1(b, b), (16 + b))[1] + 7, total(build(6, h1(b, a))))
    return 17


def main():
    y = ap(lambda x: x * ((11 & 9) if (3 if 4 < 13 else 5) < 0 else total(build(4, 11))) + 2, 1)
    f = lambda x: x * (pair(y, y)[1] * h2(14, 11)) + y
    l = build(4, y)
    return ((h2(10, (y if 11 < y else 3)) * pair(total(build(0, 8)), y)[1]), total(build(4, ap(lambda x: x * y + 3, y))), f((h2(10, (y if 11 < y else 3)) * pair(total(build(0, 8)), y)[1])) + f(y), total(l) + total(l))
