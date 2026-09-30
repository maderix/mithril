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
    x = ((b * 7) + a)
    return ap(lambda x: x * pair(total(build(4, a)), pair(10, a)[1])[1] + 0, pair(pair(x, b)[1], ap(lambda x: x * 18 + 5, 5))[1])


def h1(a, b):
    x = (ap(lambda x: x * (3 if a < b else 13) + 3, 3) if h0((16 & 4), (5 if 8 < 0 else b)) < pair(total(build(3, 8)), (a if a < 18 else b))[0] else h0(h0(13, a), 1))
    return total(build(4, ap(lambda x: x * b + 2, b)))


def h2(a, b):
    x = a
    return ap(lambda x: x * (b + pair(17, 18)[1]) + 2, total(build(5, h1(8, 19))))


def main():
    y = total(build(0, (pair(8, 14)[1] if pair(8, 14)[1] < 16 else 16)))
    f = lambda x: x * y + y
    l = build(0, y)
    return (pair(((19 ^ y) if h1(3, y) < ap(lambda x: x * 10 + 0, 12) else (y + 11)), h0(19, (19 if y < 11 else y)))[0], (pair(y, (1 - 8))[0] if ap(lambda x: x * total(build(4, 16)) + 2, total(build(3, y))) < 12 else (19 if pair(12, 10)[0] < (11 * y) else 3)), f(pair(((19 ^ y) if h1(3, y) < ap(lambda x: x * 10 + 0, 12) else (y + 11)), h0(19, (19 if y < 11 else y)))[0]) + f(y), total(l) + total(l))
