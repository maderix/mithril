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
    x = 19
    return total(build(5, ap(lambda x: x * pair(9, x)[1] + 3, x)))


def h1(a, b):
    x = (total(build(1, (13 if a < a else a))) & (15 if (17 if 5 < b else a) < (7 * 6) else (b * a)))
    return pair(ap(lambda x: x * total(build(2, 11)) + 1, (x if 12 < x else a)), 2)[1]


def h2(a, b):
    x = a
    return (pair(h1(7, b), (x & 0))[0] & total(build(4, (8 + a))))


def main():
    y = 4
    f = lambda x: x * (pair(y, y)[0] if pair(16, y)[1] < (5 ^ y) else pair(y, y)[1]) + y
    l = build(2, y)
    return (h1(y, y), pair(h0(ap(lambda x: x * y + 7, 11), (y if y < y else 10)), (pair(y, 8)[1] if 18 < total(build(0, y)) else pair(6, 5)[0]))[1], f(h1(y, y)) + f(y), total(l) + total(l))
