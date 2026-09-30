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
    x = 14
    return 6


def h1(a, b):
    x = total(build(1, total(build(0, h0(b, 12)))))
    return pair((12 if b < total(build(5, b)) else (9 if x < x else 9)), (total(build(3, 12)) if total(build(3, a)) < (x if x < 5 else 15) else 10))[0]


def h2(a, b):
    x = a
    return ((h1(x, a) & (b if x < 8 else 16)) if pair(ap(lambda x: x * x + 5, b), 14)[1] < pair(15, (x if 14 < 13 else 5))[0] else h1(a, h1(16, 8)))


def main():
    y = pair(12, h0(10, (2 * 13)))[0]
    f = lambda x: x * (h0(19, 19) if (13 - 1) < 0 else (19 if y < 3 else y)) + y
    l = build(1, y)
    return (ap(lambda x: x * total(build(3, pair(y, 2)[0])) + 3, (total(build(5, y)) if (4 if y < y else y) < y else total(build(6, 10)))), 17, f(ap(lambda x: x * total(build(3, pair(y, 2)[0])) + 3, (total(build(5, y)) if (4 if y < y else y) < y else total(build(6, 10))))) + f(y), total(l) + total(l))
