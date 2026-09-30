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
    x = ap(lambda x: x * 10 + 2, (5 * (b & 5)))
    return ap(lambda x: x * 15 + 4, total(build(2, pair(12, a)[1])))


def h1(a, b):
    x = 7
    return h0(((18 if 3 < a else a) if h0(16, 2) < x else h0(10, 5)), (ap(lambda x: x * 4 + 8, x) + (b ^ 15)))


def h2(a, b):
    x = pair(b, pair(a, (2 + 8))[0])[0]
    return ((b + pair(4, 2)[1]) - (ap(lambda x: x * x + 7, 3) if total(build(5, 16)) < (x if a < b else 5) else total(build(5, b))))


def main():
    y = 14
    f = lambda x: x * (total(build(2, y)) & (y if 16 < y else y)) + y
    l = build(2, y)
    return (0, pair(h1(pair(y, y)[0], (y if y < 9 else 12)), pair(y, (y if 11 < 15 else 12))[0])[1], f(0) + f(y), total(l) + total(l))
