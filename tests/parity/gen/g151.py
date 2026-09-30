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
    x = (total(build(6, 11)) if 1 < (14 if ap(lambda x: x * a + 7, a) < a else 13) else pair(9, ap(lambda x: x * b + 4, 19))[0])
    return (7 & total(build(2, (1 if b < 10 else b))))


def h1(a, b):
    x = ((total(build(2, 10)) if 18 < ap(lambda x: x * 4 + 0, 6) else (a if 5 < b else a)) & (8 + pair(9, b)[1]))
    return pair(total(build(5, (b if 19 < x else 0))), (total(build(1, 17)) if h0(a, x) < 2 else x))[0]


def h2(a, b):
    x = ap(lambda x: x * (pair(11, 15)[0] & (13 - b)) + 5, ap(lambda x: x * pair(7, 5)[1] + 2, total(build(5, 3))))
    return pair(pair(a, pair(a, a)[0])[0], ap(lambda x: x * h1(b, 8) + 2, ap(lambda x: x * x + 7, 14)))[1]


def main():
    y = 18
    f = lambda x: x * pair(total(build(0, 11)), (y if 18 < y else 1))[1] + y
    l = build(6, y)
    return (3, pair(ap(lambda x: x * total(build(1, 16)) + 5, h0(y, y)), pair(9, 19)[1])[0], f(3) + f(y), total(l) + total(l))
