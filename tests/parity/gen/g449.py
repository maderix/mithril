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
    x = 13
    return pair(((2 if 17 < 14 else a) if x < (12 + 11) else 5), b)[1]


def h1(a, b):
    x = total(build(6, 18))
    return x


def h2(a, b):
    x = (ap(lambda x: x * a + 6, ap(lambda x: x * 19 + 2, 7)) if ap(lambda x: x * h1(1, b) + 3, pair(9, 3)[1]) < (total(build(1, b)) * pair(a, 17)[1]) else h0((17 if 15 < a else b), h1(17, b)))
    return pair(ap(lambda x: x * ap(lambda x: x * x + 0, x) + 5, total(build(3, b))), (pair(18, 6)[1] if ap(lambda x: x * b + 0, 8) < x else 13))[1]


def main():
    y = 12
    f = lambda x: x * y + y
    l = build(0, y)
    return ((y if 12 < total(build(4, (y if 17 < y else y))) else y), y, f((y if 12 < total(build(4, (y if 17 < y else y))) else y)) + f(y), total(l) + total(l))
