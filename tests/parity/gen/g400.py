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
    x = 18
    return 17


def h1(a, b):
    x = ap(lambda x: x * 17 + 0, 2)
    return total(build(5, b))


def h2(a, b):
    x = total(build(1, a))
    return pair(total(build(0, ap(lambda x: x * x + 0, b))), total(build(5, (x + a))))[0]


def main():
    y = (pair(ap(lambda x: x * 8 + 7, 1), ap(lambda x: x * 9 + 1, 19))[1] & (5 & total(build(0, 9))))
    f = lambda x: x * ap(lambda x: x * (y & y) + 3, (19 if 16 < y else y)) + y
    l = build(1, y)
    return (h1(pair(ap(lambda x: x * y + 8, y), h1(3, 11))[0], total(build(1, pair(y, 13)[0]))), ap(lambda x: x * h2(h0(1, 6), (y & y)) + 8, h2(ap(lambda x: x * 9 + 3, 4), h0(13, y))), f(h1(pair(ap(lambda x: x * y + 8, y), h1(3, 11))[0], total(build(1, pair(y, 13)[0])))) + f(y), total(l) + total(l))
