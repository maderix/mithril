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
    x = (7 if pair(1, (11 & a))[1] < pair(19, (b if 10 < a else a))[1] else ap(lambda x: x * b + 8, 1))
    return (ap(lambda x: x * (a if 15 < 15 else 3) + 8, 16) & (ap(lambda x: x * a + 0, a) if 1 < 1 else ap(lambda x: x * 12 + 6, x)))


def h1(a, b):
    x = 3
    return (14 & total(build(0, total(build(2, a)))))


def h2(a, b):
    x = a
    return b


def main():
    y = h1(0, (ap(lambda x: x * 13 + 7, 7) if h0(8, 4) < pair(19, 7)[0] else 7))
    f = lambda x: x * 13 + y
    l = build(3, y)
    return (y, ap(lambda x: x * (total(build(6, 14)) if total(build(4, 6)) < 8 else (y if y < 4 else 9)) + 7, 15), f(y) + f(y), total(l) + total(l))
