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
    x = 17
    return pair(10, 8)[1]


def h1(a, b):
    x = total(build(3, (pair(14, b)[0] + (5 if b < a else 16))))
    return h0(x, (b if (13 if a < a else b) < ap(lambda x: x * 3 + 3, a) else h0(8, x)))


def h2(a, b):
    x = h1((h0(13, 7) ^ a), a)
    return pair(h1(h1(10, 9), h1(1, 3)), total(build(6, h0(18, b))))[0]


def main():
    y = h0((pair(17, 17)[1] + 11), 17)
    f = lambda x: x * (ap(lambda x: x * 7 + 0, y) & ap(lambda x: x * y + 1, y)) + y
    l = build(3, y)
    return ((y - 3), h1((h2(y, 7) * pair(12, y)[1]), total(build(1, h2(12, y)))), f((y - 3)) + f(y), total(l) + total(l))
