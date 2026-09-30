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
    x = 6
    return x


def h1(a, b):
    x = total(build(2, total(build(6, (4 ^ 0)))))
    return ap(lambda x: x * ((x if a < b else b) & (0 if 8 < a else x)) + 5, 8)


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * pair(b, a)[0] + 2, b) + 7, ((13 * b) if h0(14, 7) < total(build(3, b)) else (a if 8 < b else 1)))
    return (a if total(build(1, x)) < b else total(build(2, (12 if b < 16 else b))))


def main():
    y = h1(h0(total(build(6, 5)), ap(lambda x: x * 12 + 5, 15)), (ap(lambda x: x * 12 + 4, 13) - (17 if 16 < 16 else 5)))
    f = lambda x: x * pair((y ^ y), y)[1] + y
    l = build(5, y)
    return (pair(9, ap(lambda x: x * (2 * y) + 6, total(build(4, 12))))[1], pair(((y if 15 < y else y) if (14 if y < 8 else 11) < (9 if y < 16 else 8) else (8 * y)), y)[0], f(pair(9, ap(lambda x: x * (2 * y) + 6, total(build(4, 12))))[1]) + f(y), total(l) + total(l))
