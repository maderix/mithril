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
    return (3 if (total(build(4, 17)) ^ 10) < a else a)


def h1(a, b):
    x = ((a if 13 < pair(5, a)[1] else pair(a, 14)[1]) if h0(total(build(3, a)), a) < (total(build(6, 7)) if pair(8, 2)[0] < (17 + b) else pair(12, a)[1]) else h0(ap(lambda x: x * b + 2, 14), (2 + 19)))
    return ((ap(lambda x: x * 1 + 2, b) if b < (b if a < b else a) else ap(lambda x: x * 3 + 3, 13)) if pair(ap(lambda x: x * x + 6, 14), 3)[1] < 0 else h0(total(build(6, 0)), 5))


def h2(a, b):
    x = a
    return (16 if (b ^ total(build(2, x))) < ((16 if x < x else 3) ^ pair(a, 5)[0]) else h1(12, (14 if 9 < 5 else b)))


def main():
    y = 8
    f = lambda x: x * y + y
    l = build(2, y)
    return (y, 17, f(y) + f(y), total(l) + total(l))
