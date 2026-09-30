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
    x = total(build(4, total(build(2, pair(2, b)[1]))))
    return b


def h1(a, b):
    x = b
    return b


def h2(a, b):
    x = total(build(5, pair(h0(9, b), 3)[0]))
    return (total(build(4, (0 * b))) - total(build(5, h1(2, a))))


def main():
    y = (((5 if 11 < 13 else 17) + (6 * 6)) ^ total(build(4, h1(0, 0))))
    f = lambda x: x * ((14 if y < 6 else y) if y < h0(y, y) else y) + y
    l = build(2, y)
    return (pair(pair(7, ap(lambda x: x * 9 + 4, 8))[0], h1(total(build(6, y)), 16))[0], 17, f(pair(pair(7, ap(lambda x: x * 9 + 4, 8))[0], h1(total(build(6, y)), 16))[0]) + f(y), total(l) + total(l))
