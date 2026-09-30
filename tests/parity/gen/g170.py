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
    x = 3
    return (6 + (2 if (3 if 13 < 4 else b) < (8 - 19) else (7 & a)))


def h1(a, b):
    x = b
    return total(build(3, (a & (7 * 6))))


def h2(a, b):
    x = total(build(0, (9 if 5 < pair(b, b)[0] else ap(lambda x: x * a + 7, b))))
    return 10


def main():
    y = (ap(lambda x: x * total(build(2, 14)) + 3, (19 ^ 9)) if total(build(6, 6)) < pair((19 if 11 < 10 else 13), (1 if 2 < 11 else 5))[0] else total(build(6, (14 & 11))))
    f = lambda x: x * pair(1, ap(lambda x: x * y + 3, 14))[1] + y
    l = build(4, y)
    return (pair(pair(total(build(4, 1)), 6)[1], (y & y))[0], h1(((y + y) - (1 - 14)), (h1(y, y) if total(build(4, y)) < h0(y, y) else 9)), f(pair(pair(total(build(4, 1)), 6)[1], (y & y))[0]) + f(y), total(l) + total(l))
