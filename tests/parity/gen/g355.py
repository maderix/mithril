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
    x = ap(lambda x: x * 15 + 3, ((a + 3) * b))
    return pair(pair(ap(lambda x: x * 9 + 5, 15), (b & 12))[1], b)[1]


def h1(a, b):
    x = (0 if 1 < h0((b * 16), pair(5, b)[0]) else 6)
    return total(build(1, ap(lambda x: x * 14 + 7, 0)))


def h2(a, b):
    x = b
    return h0((h0(x, x) if pair(0, 0)[1] < x else total(build(5, x))), ap(lambda x: x * h1(x, 5) + 2, 16))


def main():
    y = ((pair(3, 7)[1] if 3 < 7 else pair(8, 13)[0]) + h0(ap(lambda x: x * 0 + 8, 15), h2(7, 14)))
    f = lambda x: x * pair(pair(7, y)[0], total(build(2, y)))[1] + y
    l = build(4, y)
    return (ap(lambda x: x * h1(total(build(3, 19)), pair(y, 17)[0]) + 6, y), (pair(ap(lambda x: x * y + 4, y), 8)[1] * total(build(4, ap(lambda x: x * y + 0, 16)))), f(ap(lambda x: x * h1(total(build(3, 19)), pair(y, 17)[0]) + 6, y)) + f(y), total(l) + total(l))
