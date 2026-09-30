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
    x = ap(lambda x: x * pair((b * b), 15)[0] + 3, pair(total(build(3, 15)), pair(b, b)[0])[1])
    return pair(ap(lambda x: x * (b if 13 < 17 else x) + 4, total(build(2, 4))), total(build(5, (5 if x < 16 else a))))[1]


def h1(a, b):
    x = total(build(3, total(build(4, (b if b < 18 else 8)))))
    return total(build(2, (x & ap(lambda x: x * x + 8, a))))


def h2(a, b):
    x = (pair(total(build(6, 17)), 8)[1] * total(build(1, pair(11, 13)[0])))
    return ap(lambda x: x * ((9 & 16) if ap(lambda x: x * x + 6, 6) < pair(x, 9)[1] else (6 - a)) + 4, (ap(lambda x: x * x + 3, 5) + (b & x)))


def main():
    y = 11
    f = lambda x: x * pair(pair(16, y)[0], pair(12, 4)[0])[0] + y
    l = build(2, y)
    return (14, h1(ap(lambda x: x * (y * y) + 8, (12 if 17 < 12 else y)), pair((19 if 18 < 19 else 16), h0(y, 8))[1]), f(14) + f(y), total(l) + total(l))
