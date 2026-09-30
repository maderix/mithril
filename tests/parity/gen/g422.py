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
    x = (total(build(1, b)) - 6)
    return (7 if ap(lambda x: x * total(build(2, a)) + 1, (x if a < 15 else 4)) < ap(lambda x: x * total(build(1, a)) + 2, (x & b)) else a)


def h1(a, b):
    x = b
    return b


def h2(a, b):
    x = pair(total(build(3, h0(2, 9))), (total(build(5, a)) if h0(b, 14) < total(build(4, 6)) else total(build(2, a))))[1]
    return pair(h0((a & 9), (15 if 18 < 3 else 7)), pair((0 - 16), b)[1])[0]


def main():
    y = pair(1, pair((3 * 12), 16)[0])[1]
    f = lambda x: x * 8 + y
    l = build(3, y)
    return (ap(lambda x: x * 15 + 6, h2(h0(5, y), pair(y, y)[1])), h1(y, (6 & total(build(0, 11)))), f(ap(lambda x: x * 15 + 6, h2(h0(5, y), pair(y, y)[1]))) + f(y), total(l) + total(l))
