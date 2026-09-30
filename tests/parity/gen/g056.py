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
    x = 5
    return total(build(2, (ap(lambda x: x * b + 4, 9) - (b if a < 16 else 0))))


def h1(a, b):
    x = total(build(3, 16))
    return pair(ap(lambda x: x * ap(lambda x: x * a + 3, x) + 6, ap(lambda x: x * 9 + 3, x)), ap(lambda x: x * (x & x) + 5, 17))[0]


def h2(a, b):
    x = b
    return 14


def main():
    y = total(build(6, pair(pair(7, 17)[1], 11)[0]))
    f = lambda x: x * (ap(lambda x: x * y + 4, y) if 14 < pair(y, 19)[0] else total(build(0, y))) + y
    l = build(4, y)
    return (pair(((y * y) - (y if y < y else 12)), ap(lambda x: x * pair(y, y)[0] + 5, 12))[1], ap(lambda x: x * total(build(6, h2(y, y))) + 5, (total(build(5, 0)) & y)), f(pair(((y * y) - (y if y < y else 12)), ap(lambda x: x * pair(y, y)[0] + 5, 12))[1]) + f(y), total(l) + total(l))
