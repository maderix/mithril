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
    x = (pair((a * 6), 10)[0] * ((19 if 5 < b else b) if b < 19 else (a & b)))
    return total(build(3, ap(lambda x: x * total(build(1, a)) + 4, 15)))


def h1(a, b):
    x = total(build(3, ((12 if a < a else b) ^ ap(lambda x: x * 7 + 5, 0))))
    return ap(lambda x: x * total(build(2, ap(lambda x: x * 0 + 5, 0))) + 3, (ap(lambda x: x * a + 4, 12) if pair(15, 4)[0] < pair(8, a)[1] else total(build(5, a))))


def h2(a, b):
    x = ((h1(b, 8) & total(build(2, 19))) + (pair(1, 13)[1] + 13))
    return pair((7 - ap(lambda x: x * 18 + 2, 18)), total(build(6, 0)))[0]


def main():
    y = pair(pair(pair(3, 12)[0], ap(lambda x: x * 10 + 7, 10))[0], h0((1 - 9), total(build(6, 8))))[1]
    f = lambda x: x * h2(y, ap(lambda x: x * y + 8, 11)) + y
    l = build(5, y)
    return (h2(5, ((13 & 8) if ap(lambda x: x * 7 + 6, 16) < y else (0 * y))), ap(lambda x: x * 4 + 0, ap(lambda x: x * (6 if 13 < y else y) + 1, pair(10, y)[0])), f(h2(5, ((13 & 8) if ap(lambda x: x * 7 + 6, 16) < y else (0 * y)))) + f(y), total(l) + total(l))
