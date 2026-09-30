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
    return pair(15, 19)[1]


def h1(a, b):
    x = pair(b, pair(total(build(3, a)), h0(7, 13))[0])[1]
    return (total(build(1, pair(b, 17)[1])) if total(build(4, h0(b, a))) < 10 else h0(a, ap(lambda x: x * b + 1, 2)))


def h2(a, b):
    x = 7
    return (14 if ap(lambda x: x * 3 + 0, b) < pair(ap(lambda x: x * a + 4, b), (19 if a < b else 17))[1] else ap(lambda x: x * pair(16, b)[0] + 6, pair(a, a)[0]))


def main():
    y = h2((11 - 0), 3)
    f = lambda x: x * y + y
    l = build(1, y)
    return ((y if (total(build(5, 3)) if y < total(build(0, y)) else h2(18, y)) < ((y if 17 < y else 14) if ap(lambda x: x * 14 + 0, 16) < (1 if y < y else y) else pair(y, 10)[1]) else 14), (y if ((y + 7) if pair(14, y)[1] < (y if 16 < y else y) else ap(lambda x: x * 7 + 4, 10)) < pair(y, h0(0, 16))[0] else y), f((y if (total(build(5, 3)) if y < total(build(0, y)) else h2(18, y)) < ((y if 17 < y else 14) if ap(lambda x: x * 14 + 0, 16) < (1 if y < y else y) else pair(y, 10)[1]) else 14)) + f(y), total(l) + total(l))
