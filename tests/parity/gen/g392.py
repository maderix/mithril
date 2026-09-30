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
    return a


def h1(a, b):
    x = (6 & h0(pair(8, 2)[0], h0(14, a)))
    return (h0((a - 1), (13 if 0 < a else 17)) & (total(build(1, x)) if (b if 13 < 1 else a) < (b if 17 < 14 else a) else pair(x, b)[1]))


def h2(a, b):
    x = b
    return pair((pair(12, x)[0] if (4 & 16) < pair(19, b)[1] else h0(a, 12)), 9)[0]


def main():
    y = 9
    f = lambda x: x * (total(build(4, 15)) if total(build(1, y)) < pair(1, 3)[0] else h0(y, y)) + y
    l = build(3, y)
    return ((pair(ap(lambda x: x * y + 8, y), pair(y, 17)[1])[1] - pair(pair(2, 8)[0], (y * 15))[1]), h2(pair(h2(18, 4), (15 & 17))[0], y), f((pair(ap(lambda x: x * y + 8, y), pair(y, 17)[1])[1] - pair(pair(2, 8)[0], (y * 15))[1])) + f(y), total(l) + total(l))
