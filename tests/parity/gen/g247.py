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
    x = (11 if (4 ^ total(build(0, b))) < 10 else 6)
    return x


def h1(a, b):
    x = 13
    return h0((13 * (3 & 3)), total(build(4, ap(lambda x: x * a + 6, 5))))


def h2(a, b):
    x = b
    return 7


def main():
    y = 5
    f = lambda x: x * h1(y, (y + y)) + y
    l = build(6, y)
    return (pair((pair(y, y)[0] if 6 < y else 9), ap(lambda x: x * (16 if y < y else 10) + 8, (10 & 0)))[0], ((ap(lambda x: x * 3 + 6, y) * (y if y < 12 else y)) ^ (pair(y, y)[0] - (y - y))), f(pair((pair(y, y)[0] if 6 < y else 9), ap(lambda x: x * (16 if y < y else 10) + 8, (10 & 0)))[0]) + f(y), total(l) + total(l))
