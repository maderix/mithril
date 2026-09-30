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
    x = ap(lambda x: x * pair(11, (5 if 18 < 12 else b))[1] + 7, (total(build(1, a)) ^ 13))
    return ap(lambda x: x * (18 if 11 < total(build(3, x)) else (5 * 3)) + 5, total(build(0, total(build(2, a)))))


def h1(a, b):
    x = (4 if ((b + b) if 1 < pair(b, 8)[0] else ap(lambda x: x * b + 1, b)) < ((4 & 10) if ap(lambda x: x * 3 + 5, a) < pair(b, b)[0] else 19) else (pair(a, a)[0] if (b * b) < total(build(3, 7)) else ap(lambda x: x * 3 + 2, 13)))
    return (a if pair(h0(19, 13), pair(7, 2)[0])[0] < b else 13)


def h2(a, b):
    x = total(build(2, ap(lambda x: x * pair(a, b)[1] + 8, a)))
    return pair(1, pair(total(build(0, x)), total(build(1, b)))[1])[0]


def main():
    y = total(build(0, pair(ap(lambda x: x * 17 + 7, 16), total(build(1, 3)))[0]))
    f = lambda x: x * 14 + y
    l = build(0, y)
    return (pair((h2(4, 12) if 4 < pair(y, 1)[1] else y), (y if total(build(6, y)) < pair(y, y)[0] else pair(y, y)[0]))[0], pair(1, total(build(3, y)))[1], f(pair((h2(4, 12) if 4 < pair(y, 1)[1] else y), (y if total(build(6, y)) < pair(y, y)[0] else pair(y, y)[0]))[0]) + f(y), total(l) + total(l))
