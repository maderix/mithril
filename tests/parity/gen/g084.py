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
    x = 9
    return a


def h1(a, b):
    x = b
    return ap(lambda x: x * (h0(19, a) * x) + 3, (total(build(2, 7)) - h0(1, 5)))


def h2(a, b):
    x = (9 - total(build(4, (15 if b < 18 else a))))
    return h0(ap(lambda x: x * b + 2, h1(a, 2)), total(build(2, pair(6, x)[1])))


def main():
    y = h1(pair(19, h1(15, 2))[0], 8)
    f = lambda x: x * 1 + y
    l = build(6, y)
    return (h2(17, ((y + y) if (17 + y) < ap(lambda x: x * y + 7, y) else (y - 18))), pair(pair((8 if y < 11 else y), pair(y, 5)[1])[1], total(build(5, pair(2, 4)[0])))[1], f(h2(17, ((y + y) if (17 + y) < ap(lambda x: x * y + 7, y) else (y - 18)))) + f(y), total(l) + total(l))
