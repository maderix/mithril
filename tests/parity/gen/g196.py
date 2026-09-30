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
    x = ap(lambda x: x * (b if (a & 3) < 9 else (7 if 11 < b else b)) + 5, (pair(0, 0)[0] if 17 < pair(13, b)[0] else (19 if 10 < b else a)))
    return (pair((12 if 13 < a else x), (x ^ 3))[0] * pair(total(build(1, x)), 11)[0])


def h1(a, b):
    x = ap(lambda x: x * total(build(5, (b if a < 1 else 3))) + 1, pair(pair(b, a)[1], ap(lambda x: x * 13 + 4, 3))[1])
    return total(build(4, h0(ap(lambda x: x * a + 4, x), (15 & b))))


def h2(a, b):
    x = h1((total(build(5, b)) & h0(b, 4)), pair(16, ap(lambda x: x * b + 1, 5))[0])
    return 1


def main():
    y = (7 ^ (ap(lambda x: x * 13 + 3, 11) if (12 * 12) < ap(lambda x: x * 19 + 7, 13) else h0(11, 12)))
    f = lambda x: x * pair((y if y < 16 else y), total(build(4, y)))[0] + y
    l = build(6, y)
    return (pair(h0((y if y < y else 18), (12 if 7 < 0 else y)), ap(lambda x: x * pair(y, y)[1] + 7, pair(y, y)[1]))[1], h0(ap(lambda x: x * (y & 15) + 0, h2(8, 9)), y), f(pair(h0((y if y < y else 18), (12 if 7 < 0 else y)), ap(lambda x: x * pair(y, y)[1] + 7, pair(y, y)[1]))[1]) + f(y), total(l) + total(l))
