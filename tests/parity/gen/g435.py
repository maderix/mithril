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
    x = 3
    return pair(total(build(0, (16 + x))), total(build(4, 9)))[0]


def h1(a, b):
    x = (total(build(4, a)) if total(build(3, 17)) < ((b if 6 < b else 4) ^ pair(b, a)[0]) else total(build(5, (18 if b < 3 else 19))))
    return (ap(lambda x: x * 16 + 4, 12) & (ap(lambda x: x * x + 0, b) + (18 + 15)))


def h2(a, b):
    x = total(build(2, 14))
    return (9 if total(build(0, b)) < pair(ap(lambda x: x * 0 + 4, a), h1(a, x))[0] else 18)


def main():
    y = ap(lambda x: x * h0(pair(7, 2)[1], (6 - 12)) + 7, (total(build(0, 3)) if (0 - 8) < h0(18, 14) else h0(9, 3)))
    f = lambda x: x * 4 + y
    l = build(1, y)
    return (ap(lambda x: x * y + 7, pair(total(build(1, y)), (10 if y < y else y))[0]), h0(h1(3, total(build(6, y))), (7 if y < total(build(2, 3)) else ap(lambda x: x * 18 + 1, 11))), f(ap(lambda x: x * y + 7, pair(total(build(1, y)), (10 if y < y else y))[0])) + f(y), total(l) + total(l))
