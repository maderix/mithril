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
    x = (a & 14)
    return ap(lambda x: x * total(build(5, 2)) + 8, ap(lambda x: x * pair(10, x)[1] + 1, 12))


def h1(a, b):
    x = (18 ^ pair(b, ap(lambda x: x * a + 2, 14))[0])
    return h0((x if h0(x, b) < h0(b, 11) else (3 + 3)), h0(a, a))


def h2(a, b):
    x = (((a if a < 9 else a) if 10 < pair(14, b)[0] else 4) if (pair(9, 12)[1] - total(build(5, 8))) < total(build(0, h0(17, b))) else h0((12 ^ 18), b))
    return h1(pair(h1(15, 5), (x if 9 < b else 11))[0], total(build(6, pair(9, a)[1])))


def main():
    y = 16
    f = lambda x: x * (h0(y, y) + ap(lambda x: x * y + 7, 9)) + y
    l = build(4, y)
    return (ap(lambda x: x * pair((6 if y < y else 0), y)[0] + 7, ap(lambda x: x * (15 + 12) + 4, y)), y, f(ap(lambda x: x * pair((6 if y < y else 0), y)[0] + 7, ap(lambda x: x * (15 + 12) + 4, y))) + f(y), total(l) + total(l))
