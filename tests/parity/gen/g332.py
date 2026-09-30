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
    x = (8 if pair(2, (9 - 11))[1] < ap(lambda x: x * 0 + 5, 17) else ap(lambda x: x * (a - b) + 4, (14 + b)))
    return total(build(5, (ap(lambda x: x * b + 1, a) if ap(lambda x: x * 12 + 0, x) < ap(lambda x: x * a + 3, a) else (b & 18))))


def h1(a, b):
    x = total(build(4, ap(lambda x: x * total(build(1, a)) + 8, (b + 12))))
    return h0((b if ap(lambda x: x * x + 1, b) < 8 else (12 if 19 < a else b)), b)


def h2(a, b):
    x = (ap(lambda x: x * 17 + 4, ap(lambda x: x * b + 5, 3)) if h1(b, 16) < total(build(5, 12)) else (total(build(6, a)) - (9 ^ 19)))
    return h1(((2 & b) * h1(b, x)), (ap(lambda x: x * a + 1, 15) if pair(6, 3)[0] < (x if 19 < 19 else 6) else total(build(2, 12))))


def main():
    y = h1(total(build(4, (10 if 7 < 13 else 14))), 10)
    f = lambda x: x * y + y
    l = build(3, y)
    return (pair(ap(lambda x: x * (y if 14 < y else 6) + 3, h1(y, y)), 15)[0], pair(y, total(build(6, h0(1, y))))[0], f(pair(ap(lambda x: x * (y if 14 < y else 6) + 3, h1(y, y)), 15)[0]) + f(y), total(l) + total(l))
