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
    x = (a if b < b else 1)
    return ((ap(lambda x: x * b + 0, 10) if 13 < (x if 13 < 2 else a) else 14) * pair((a if 8 < 8 else b), 5)[1])


def h1(a, b):
    x = h0((pair(18, b)[1] - a), pair(ap(lambda x: x * a + 7, 3), ap(lambda x: x * a + 3, 17))[1])
    return ((7 if ap(lambda x: x * b + 2, a) < ap(lambda x: x * 10 + 6, 5) else ap(lambda x: x * 6 + 0, 17)) - h0((b if a < 19 else 3), h0(a, a)))


def h2(a, b):
    x = total(build(1, h0((16 & b), h0(b, a))))
    return (ap(lambda x: x * (a + b) + 0, a) if (1 - (a + 14)) < 17 else h0(1, b))


def main():
    y = h0(1, (2 * h2(14, 5)))
    f = lambda x: x * total(build(2, h1(0, 0))) + y
    l = build(3, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
