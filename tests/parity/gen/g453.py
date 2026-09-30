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
    x = (((b if a < b else b) if (1 if a < b else 9) < pair(15, 17)[1] else (a if a < b else 0)) if ap(lambda x: x * 0 + 0, 17) < ap(lambda x: x * ap(lambda x: x * a + 3, 9) + 5, 5) else (a * ap(lambda x: x * a + 1, a)))
    return (ap(lambda x: x * 14 + 2, (6 if b < a else 8)) ^ 3)


def h1(a, b):
    x = a
    return h0(a, ((15 if 4 < 14 else b) if h0(8, x) < b else pair(x, 9)[1]))


def h2(a, b):
    x = (h1((8 - a), 13) * h0(ap(lambda x: x * a + 0, 11), pair(17, b)[0]))
    return (12 ^ ((b if a < 3 else 0) * h1(b, 2)))


def main():
    y = 13
    f = lambda x: x * y + y
    l = build(1, y)
    return ((y ^ ((4 & 5) if h0(y, 11) < (19 if 3 < 4 else y) else (7 if y < y else y))), h0(total(build(6, h1(y, y))), (total(build(4, 7)) if y < y else (y if 19 < y else 3))), f((y ^ ((4 & 5) if h0(y, 11) < (19 if 3 < 4 else y) else (7 if y < y else y)))) + f(y), total(l) + total(l))
