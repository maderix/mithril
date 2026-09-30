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
    x = 15
    return pair((b if (x if b < 10 else 18) < pair(1, 8)[0] else ap(lambda x: x * a + 8, 0)), pair(ap(lambda x: x * 11 + 2, a), 5)[0])[0]


def h1(a, b):
    x = 10
    return 6


def h2(a, b):
    x = 19
    return h0((pair(x, b)[1] if h1(5, b) < ap(lambda x: x * 12 + 1, 1) else pair(5, 3)[1]), ap(lambda x: x * 4 + 4, b))


def main():
    y = 18
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * y + 6, 1) + 3, total(build(1, y))) + y
    l = build(3, y)
    return (y, pair(h1((6 if y < y else 14), total(build(1, 8))), (ap(lambda x: x * 4 + 1, y) ^ h0(y, 14)))[1], f(y) + f(y), total(l) + total(l))
