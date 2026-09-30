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
    return (total(build(2, (10 if 9 < 0 else b))) if pair(2, pair(13, 19)[0])[0] < pair((b * 1), b)[0] else ((b - 2) ^ total(build(0, 10))))


def h1(a, b):
    x = a
    return (2 if total(build(5, (a + a))) < h0((a ^ 6), 19) else ap(lambda x: x * (b if b < x else a) + 7, x))


def h2(a, b):
    x = h0((pair(7, 16)[0] - b), (total(build(5, 17)) if ap(lambda x: x * a + 1, a) < 4 else pair(4, b)[1]))
    return (ap(lambda x: x * pair(a, 3)[1] + 1, 3) * 19)


def main():
    y = ((pair(8, 2)[1] * 4) * 11)
    f = lambda x: x * 10 + y
    l = build(0, y)
    return (4, 5, f(4) + f(y), total(l) + total(l))
