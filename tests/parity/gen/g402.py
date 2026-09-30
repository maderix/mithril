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
    x = (total(build(3, pair(a, 0)[0])) + ((a ^ 14) if 16 < (a ^ a) else ap(lambda x: x * b + 2, 3)))
    return 18


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * pair(b, a)[1] + 4, (a if 1 < a else 19)) + 1, pair(a, total(build(2, 4)))[0])
    return (total(build(4, ap(lambda x: x * 10 + 1, 14))) * 15)


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * (15 & 16) + 8, ap(lambda x: x * a + 1, 12)) + 1, pair(h0(16, b), (b if a < 18 else 6))[1])
    return total(build(4, pair(ap(lambda x: x * x + 3, 5), total(build(5, x)))[1]))


def main():
    y = h1(0, ap(lambda x: x * pair(15, 15)[0] + 5, h0(18, 9)))
    f = lambda x: x * total(build(4, 3)) + y
    l = build(1, y)
    return (total(build(6, (pair(9, 15)[0] if (6 if y < y else 1) < pair(6, 11)[1] else (6 if y < y else y)))), pair(pair(y, y)[1], ap(lambda x: x * pair(15, 0)[0] + 1, 3))[0], f(total(build(6, (pair(9, 15)[0] if (6 if y < y else 1) < pair(6, 11)[1] else (6 if y < y else y))))) + f(y), total(l) + total(l))
