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
    x = ((total(build(6, a)) + ap(lambda x: x * a + 8, 10)) + (7 if b < ap(lambda x: x * 18 + 0, b) else 12))
    return total(build(3, x))


def h1(a, b):
    x = total(build(2, 13))
    return total(build(5, (13 * 16)))


def h2(a, b):
    x = 9
    return h1(14, (pair(4, 12)[0] - total(build(5, x))))


def main():
    y = ((pair(2, 13)[0] - ap(lambda x: x * 3 + 5, 6)) if pair(pair(9, 1)[1], pair(0, 0)[1])[0] < (pair(8, 1)[0] - ap(lambda x: x * 12 + 4, 2)) else ((5 + 13) + 11))
    f = lambda x: x * 12 + y
    l = build(2, y)
    return (h1(ap(lambda x: x * 3 + 5, total(build(5, 3))), ap(lambda x: x * total(build(0, 2)) + 4, ap(lambda x: x * y + 3, y))), ((ap(lambda x: x * y + 4, 17) if (15 if y < y else y) < pair(11, 12)[0] else (y if 1 < y else 5)) * (pair(y, y)[0] ^ pair(7, 2)[0])), f(h1(ap(lambda x: x * 3 + 5, total(build(5, 3))), ap(lambda x: x * total(build(0, 2)) + 4, ap(lambda x: x * y + 3, y)))) + f(y), total(l) + total(l))
