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
    x = (total(build(3, 19)) if ap(lambda x: x * (a ^ 18) + 0, 0) < 1 else ((a & a) if 4 < total(build(3, a)) else total(build(6, 9))))
    return (pair(ap(lambda x: x * 10 + 4, 1), (x + a))[1] & pair(b, (2 ^ 10))[1])


def h1(a, b):
    x = (pair(total(build(1, 6)), (a if b < 18 else b))[0] - a)
    return ap(lambda x: x * 10 + 3, total(build(3, total(build(1, x)))))


def h2(a, b):
    x = (h0((a if 13 < 2 else 11), 18) * a)
    return (total(build(4, (a ^ a))) ^ h0(total(build(3, 15)), ap(lambda x: x * b + 5, 3)))


def main():
    y = 10
    f = lambda x: x * pair(pair(7, y)[0], ap(lambda x: x * 11 + 2, 16))[1] + y
    l = build(4, y)
    return ((ap(lambda x: x * h1(y, y) + 0, (y if y < 2 else 9)) if y < total(build(5, ap(lambda x: x * 11 + 6, y))) else (y - pair(y, 13)[1])), pair((total(build(0, 17)) ^ ap(lambda x: x * 19 + 0, 5)), (total(build(0, 3)) if pair(y, y)[0] < total(build(1, 16)) else (y ^ y)))[1], f((ap(lambda x: x * h1(y, y) + 0, (y if y < 2 else 9)) if y < total(build(5, ap(lambda x: x * 11 + 6, y))) else (y - pair(y, 13)[1]))) + f(y), total(l) + total(l))
