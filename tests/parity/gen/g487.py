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
    x = 2
    return b


def h1(a, b):
    x = a
    return (3 * total(build(5, total(build(4, 0)))))


def h2(a, b):
    x = ap(lambda x: x * (pair(15, b)[0] if b < pair(13, b)[1] else ap(lambda x: x * b + 1, 14)) + 3, a)
    return (total(build(0, b)) & 3)


def main():
    y = (total(build(4, pair(15, 5)[0])) & h0(h2(11, 12), (14 if 14 < 19 else 12)))
    f = lambda x: x * pair(19, (y * y))[1] + y
    l = build(6, y)
    return ((h2(13, h0(15, 16)) - pair((8 + y), h0(0, y))[0]), ((9 + y) & ((y if y < 17 else 10) if pair(y, y)[0] < (12 - 19) else pair(y, 17)[1])), f((h2(13, h0(15, 16)) - pair((8 + y), h0(0, y))[0])) + f(y), total(l) + total(l))
