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
    x = pair(pair(19, 13)[0], 1)[1]
    return pair(total(build(2, ap(lambda x: x * x + 2, b))), pair(pair(6, 3)[1], 1)[0])[0]


def h1(a, b):
    x = (h0(h0(b, b), h0(a, 19)) + 3)
    return x


def h2(a, b):
    x = (total(build(4, (a + a))) ^ (ap(lambda x: x * 9 + 4, a) * a))
    return (ap(lambda x: x * 4 + 2, b) if a < b else (total(build(4, 6)) ^ x))


def main():
    y = (ap(lambda x: x * (11 ^ 6) + 0, (7 & 14)) if ap(lambda x: x * h0(17, 11) + 0, h1(17, 16)) < 4 else 5)
    f = lambda x: x * 10 + y
    l = build(0, y)
    return (pair(pair((y ^ y), 0)[1], (total(build(2, y)) if ap(lambda x: x * 12 + 1, y) < 4 else (y if 1 < y else 19)))[0], ap(lambda x: x * ap(lambda x: x * 0 + 0, (16 * y)) + 0, ap(lambda x: x * total(build(2, y)) + 3, (y * y))), f(pair(pair((y ^ y), 0)[1], (total(build(2, y)) if ap(lambda x: x * 12 + 1, y) < 4 else (y if 1 < y else 19)))[0]) + f(y), total(l) + total(l))
