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
    x = ((5 ^ total(build(1, b))) if pair(total(build(3, 19)), (1 if 18 < 16 else a))[1] < b else (total(build(5, 8)) * a))
    return total(build(6, pair(pair(3, x)[0], (0 * b))[1]))


def h1(a, b):
    x = 16
    return h0(h0((a ^ x), pair(8, b)[1]), b)


def h2(a, b):
    x = (total(build(2, pair(11, 14)[1])) if h0(ap(lambda x: x * 12 + 7, 10), (a if a < a else b)) < ((a if 2 < 5 else b) if total(build(4, b)) < a else (a if 19 < b else a)) else 11)
    return b


def main():
    y = h1(((14 + 15) if 6 < 1 else 0), 19)
    f = lambda x: x * y + y
    l = build(2, y)
    return (((y if h1(10, 19) < total(build(0, y)) else pair(y, y)[0]) if y < pair(pair(6, y)[1], h2(4, y))[0] else (total(build(0, 15)) & 4)), y, f(((y if h1(10, 19) < total(build(0, y)) else pair(y, y)[0]) if y < pair(pair(6, y)[1], h2(4, y))[0] else (total(build(0, 15)) & 4))) + f(y), total(l) + total(l))
