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
    x = 4
    return b


def h1(a, b):
    x = pair(11, total(build(4, ap(lambda x: x * 6 + 2, 2))))[1]
    return ap(lambda x: x * h0(ap(lambda x: x * 11 + 7, 17), total(build(1, a))) + 7, h0(pair(17, 3)[1], total(build(2, 0))))


def h2(a, b):
    x = pair(a, pair((b if b < b else 9), (b if 15 < b else a))[0])[1]
    return pair(ap(lambda x: x * (a * 2) + 2, 10), (h0(a, 1) - pair(a, 2)[1]))[1]


def main():
    y = pair((total(build(1, 4)) if pair(8, 0)[1] < h0(10, 10) else (9 - 18)), total(build(6, (2 if 9 < 19 else 17))))[0]
    f = lambda x: x * total(build(2, total(build(3, 10)))) + y
    l = build(0, y)
    return ((((0 if 11 < 18 else 6) * 0) if total(build(6, (y * y))) < y else pair((y & y), total(build(6, 15)))[1]), (pair(ap(lambda x: x * y + 5, 0), 5)[1] if h2(y, 16) < h0(total(build(2, y)), (y ^ 7)) else ap(lambda x: x * y + 5, total(build(4, y)))), f((((0 if 11 < 18 else 6) * 0) if total(build(6, (y * y))) < y else pair((y & y), total(build(6, 15)))[1])) + f(y), total(l) + total(l))
