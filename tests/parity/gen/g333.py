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
    x = pair(1, (ap(lambda x: x * a + 5, a) if a < a else 2))[0]
    return total(build(5, pair((19 if 4 < a else 18), 8)[1]))


def h1(a, b):
    x = h0(h0(total(build(5, 5)), ap(lambda x: x * a + 7, b)), ap(lambda x: x * total(build(4, 19)) + 1, ap(lambda x: x * 5 + 0, 7)))
    return 2


def h2(a, b):
    x = b
    return b


def main():
    y = 16
    f = lambda x: x * h0(7, pair(y, 11)[1]) + y
    l = build(5, y)
    return (h2(((2 if 15 < y else 6) & ap(lambda x: x * y + 4, 10)), 5), pair(((y * y) if h1(y, 17) < ap(lambda x: x * 0 + 1, 14) else total(build(1, y))), 2)[0], f(h2(((2 if 15 < y else 6) & ap(lambda x: x * y + 4, 10)), 5)) + f(y), total(l) + total(l))
