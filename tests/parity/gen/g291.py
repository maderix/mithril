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
    x = 16
    return a


def h1(a, b):
    x = (((3 * b) if a < (b if b < 11 else b) else pair(14, 17)[0]) & b)
    return ((h0(a, 8) ^ ap(lambda x: x * x + 3, 5)) if h0(total(build(3, 3)), (b if 5 < x else 9)) < (h0(6, 8) - 16) else total(build(1, total(build(1, 2)))))


def h2(a, b):
    x = ap(lambda x: x * (h1(17, a) if 11 < b else (b ^ a)) + 2, total(build(4, total(build(5, a)))))
    return h0(a, ap(lambda x: x * h1(b, b) + 8, total(build(1, 2))))


def main():
    y = total(build(0, h0(total(build(0, 14)), 18)))
    f = lambda x: x * ((7 if y < 15 else y) if (y - y) < h1(y, 4) else (15 * y)) + y
    l = build(1, y)
    return (((ap(lambda x: x * y + 3, 2) & ap(lambda x: x * y + 7, 17)) - (y if total(build(5, y)) < total(build(6, 14)) else 1)), ap(lambda x: x * y + 2, pair((y * 10), y)[0]), f(((ap(lambda x: x * y + 3, 2) & ap(lambda x: x * y + 7, 17)) - (y if total(build(5, y)) < total(build(6, 14)) else 1))) + f(y), total(l) + total(l))
