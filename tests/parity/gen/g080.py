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
    x = 18
    return ap(lambda x: x * ap(lambda x: x * 4 + 7, (3 if b < b else x)) + 6, a)


def h1(a, b):
    x = h0(total(build(6, (17 & b))), pair((a if 1 < 9 else 14), 9)[0])
    return a


def h2(a, b):
    x = (total(build(2, total(build(1, 17)))) if b < ((4 ^ b) if h0(0, b) < (b if 12 < a else 6) else 13) else ((b if 16 < b else a) if total(build(0, a)) < (a if 19 < 10 else b) else pair(12, 5)[0]))
    return (18 if ap(lambda x: x * pair(b, a)[1] + 3, (10 ^ 13)) < (pair(14, x)[0] + h0(18, 2)) else (x if h0(3, 1) < (x if 10 < 12 else 9) else (12 if 14 < x else b)))


def main():
    y = h0((pair(11, 10)[0] ^ (9 * 1)), 0)
    f = lambda x: x * ap(lambda x: x * 10 + 0, pair(y, y)[1]) + y
    l = build(6, y)
    return (6, total(build(2, (y if total(build(1, 18)) < h0(11, 4) else h2(y, 9)))), f(6) + f(y), total(l) + total(l))
