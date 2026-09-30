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
    x = ap(lambda x: x * ap(lambda x: x * (8 * 11) + 2, total(build(1, 9))) + 3, a)
    return 12


def h1(a, b):
    x = (b if ap(lambda x: x * a + 4, b) < a else ((14 if a < 3 else b) + (16 ^ 5)))
    return ap(lambda x: x * pair(total(build(2, 0)), x)[1] + 1, total(build(2, ap(lambda x: x * a + 2, x))))


def h2(a, b):
    x = 3
    return pair(h0((19 * 3), (8 * x)), h0(total(build(0, x)), (14 if b < b else 13)))[0]


def main():
    y = (11 if ap(lambda x: x * h0(19, 3) + 0, total(build(6, 19))) < pair(h0(12, 15), 17)[1] else 12)
    f = lambda x: x * (18 + y) + y
    l = build(3, y)
    return (h2(3, y), ap(lambda x: x * (ap(lambda x: x * 18 + 8, y) if h0(12, y) < (8 & y) else 17) + 6, y), f(h2(3, y)) + f(y), total(l) + total(l))
