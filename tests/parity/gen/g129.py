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
    x = 13
    return total(build(0, total(build(2, b))))


def h1(a, b):
    x = (h0(total(build(3, 19)), (a & a)) & total(build(5, a)))
    return total(build(0, h0(h0(a, b), h0(x, 3))))


def h2(a, b):
    x = h1(ap(lambda x: x * (a if b < 11 else 11) + 6, pair(10, a)[0]), 10)
    return ((total(build(0, 13)) if (a if 1 < 10 else 7) < ap(lambda x: x * x + 6, a) else b) if h0(10, ap(lambda x: x * 10 + 7, 5)) < h0(15, a) else h0((x * 7), h0(x, b)))


def main():
    y = 2
    f = lambda x: x * pair(total(build(3, 17)), (y - 2))[0] + y
    l = build(4, y)
    return ((11 if pair(h0(y, y), (13 if 18 < y else 0))[1] < y else ((8 & 12) + ap(lambda x: x * y + 7, y))), (19 if total(build(0, ap(lambda x: x * y + 4, y))) < ap(lambda x: x * total(build(4, 8)) + 8, (2 if y < y else y)) else y), f((11 if pair(h0(y, y), (13 if 18 < y else 0))[1] < y else ((8 & 12) + ap(lambda x: x * y + 7, y)))) + f(y), total(l) + total(l))
