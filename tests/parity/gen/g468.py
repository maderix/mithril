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
    return (((a if x < 19 else 9) if (a - a) < 11 else total(build(5, a))) if 13 < 16 else ((6 if 8 < b else a) & (15 & 5)))


def h1(a, b):
    x = ((h0(2, 5) if a < (1 if 0 < a else b) else (a if a < a else 1)) if ap(lambda x: x * (9 + 18) + 8, h0(12, b)) < pair(ap(lambda x: x * a + 2, 13), (a if b < 12 else 12))[0] else 17)
    return total(build(4, h0(ap(lambda x: x * b + 8, 2), h0(16, a))))


def h2(a, b):
    x = (ap(lambda x: x * h0(14, b) + 4, a) if h1((5 if a < 14 else b), total(build(0, b))) < (ap(lambda x: x * a + 2, 14) & h0(a, b)) else pair(b, 1)[0])
    return ap(lambda x: x * h0(h0(a, 4), (a if a < 19 else b)) + 3, (h0(x, 1) if a < total(build(4, 11)) else (a + b)))


def main():
    y = 8
    f = lambda x: x * ap(lambda x: x * 6 + 4, y) + y
    l = build(3, y)
    return (13, y, f(13) + f(y), total(l) + total(l))
