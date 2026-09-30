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
    x = pair(3, ap(lambda x: x * b + 1, (14 * b)))[0]
    return (3 + x)


def h1(a, b):
    x = ((ap(lambda x: x * a + 8, 16) if total(build(0, b)) < pair(2, 6)[1] else pair(10, 0)[1]) + h0((16 if b < 2 else b), (b if 14 < 1 else a)))
    return x


def h2(a, b):
    x = a
    return ap(lambda x: x * pair(a, total(build(3, 17)))[0] + 5, h1(pair(a, a)[0], ap(lambda x: x * 8 + 3, 9)))


def main():
    y = (h1(18, h0(9, 5)) * pair((13 + 17), total(build(0, 15)))[1])
    f = lambda x: x * (total(build(2, 17)) if y < total(build(6, y)) else (2 if 7 < 16 else y)) + y
    l = build(2, y)
    return (ap(lambda x: x * total(build(3, total(build(3, y)))) + 8, pair(18, (12 - y))[0]), (y ^ h0(pair(y, y)[1], y)), f(ap(lambda x: x * total(build(3, total(build(3, y)))) + 8, pair(18, (12 - y))[0])) + f(y), total(l) + total(l))
