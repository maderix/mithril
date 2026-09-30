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
    x = pair(total(build(4, (a - a))), ap(lambda x: x * 18 + 2, ap(lambda x: x * b + 6, 18)))[1]
    return 18


def h1(a, b):
    x = a
    return h0((pair(b, x)[1] * x), ap(lambda x: x * (8 if a < 18 else b) + 2, (12 if x < 19 else a)))


def h2(a, b):
    x = (total(build(2, (b ^ a))) * (total(build(0, 2)) + pair(10, 4)[1]))
    return 5


def main():
    y = pair(h0((17 + 3), 7), ((7 * 12) if 16 < h1(11, 16) else pair(17, 6)[1]))[1]
    f = lambda x: x * (2 + total(build(0, y))) + y
    l = build(4, y)
    return (pair(ap(lambda x: x * 1 + 5, h0(5, y)), h0(2, pair(y, 0)[0]))[1], pair((17 if (17 if y < y else y) < ap(lambda x: x * 1 + 0, y) else h1(y, 0)), h1((18 if 11 < 8 else 11), pair(16, 1)[0]))[0], f(pair(ap(lambda x: x * 1 + 5, h0(5, y)), h0(2, pair(y, 0)[0]))[1]) + f(y), total(l) + total(l))
