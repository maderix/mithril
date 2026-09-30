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
    return (pair(total(build(6, x)), 3)[0] * 10)


def h1(a, b):
    x = b
    return a


def h2(a, b):
    x = (ap(lambda x: x * (b - a) + 3, (8 - 2)) + total(build(2, h0(4, a))))
    return (6 & pair((1 * a), h1(x, a))[1])


def main():
    y = 6
    f = lambda x: x * pair(total(build(6, y)), pair(9, 14)[1])[0] + y
    l = build(6, y)
    return (y, (pair(pair(y, y)[0], 4)[0] if pair((y if y < 18 else y), total(build(3, y)))[1] < (y if (13 + 16) < h0(y, y) else h2(4, 18)) else pair(total(build(2, y)), 9)[0]), f(y) + f(y), total(l) + total(l))
