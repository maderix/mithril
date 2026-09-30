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
    x = a
    return pair(16, pair((x if b < 10 else 15), ap(lambda x: x * 1 + 6, 14))[0])[0]


def h1(a, b):
    x = a
    return b


def h2(a, b):
    x = (h0(a, (8 if 2 < a else b)) if a < (9 if pair(b, 1)[0] < ap(lambda x: x * a + 7, 3) else total(build(2, b))) else (pair(4, b)[0] + (a if 1 < a else 11)))
    return 6


def main():
    y = ((h1(15, 15) if total(build(5, 13)) < (12 if 8 < 7 else 8) else ap(lambda x: x * 8 + 4, 15)) - 10)
    f = lambda x: x * (h2(y, 14) if h0(12, y) < pair(10, y)[0] else h2(8, y)) + y
    l = build(6, y)
    return (h1(((y - 16) if total(build(3, 1)) < (19 if y < y else y) else (17 - 3)), total(build(1, (13 if y < 9 else y)))), y, f(h1(((y - 16) if total(build(3, 1)) < (19 if y < y else y) else (17 - 3)), total(build(1, (13 if y < 9 else y))))) + f(y), total(l) + total(l))
