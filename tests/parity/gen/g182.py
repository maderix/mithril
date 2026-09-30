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
    x = (ap(lambda x: x * 17 + 1, 13) * total(build(2, (9 - b))))
    return 10


def h1(a, b):
    x = (13 + ap(lambda x: x * h0(b, b) + 6, h0(9, a)))
    return h0(h0(pair(x, 8)[0], total(build(1, x))), total(build(5, pair(a, 15)[1])))


def h2(a, b):
    x = ap(lambda x: x * total(build(0, (16 if 4 < 4 else 14))) + 7, pair(a, (9 * 0))[0])
    return 6


def main():
    y = (pair(h2(2, 7), total(build(4, 16)))[1] if h2(total(build(6, 19)), pair(0, 14)[0]) < pair((7 if 4 < 4 else 14), (12 & 12))[1] else h0((12 if 19 < 1 else 11), ap(lambda x: x * 5 + 1, 18)))
    f = lambda x: x * total(build(0, y)) + y
    l = build(1, y)
    return (h0(pair(3, 12)[1], h2(2, 2)), h0(pair(17, total(build(4, 1)))[1], ap(lambda x: x * 13 + 0, pair(y, y)[0])), f(h0(pair(3, 12)[1], h2(2, 2))) + f(y), total(l) + total(l))
