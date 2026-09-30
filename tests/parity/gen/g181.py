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
    x = 19
    return pair(((2 - x) if (x ^ 19) < a else 0), ap(lambda x: x * total(build(4, 18)) + 5, (a if x < a else 12)))[1]


def h1(a, b):
    x = pair(h0(13, a), 15)[1]
    return (total(build(5, total(build(4, 15)))) if 1 < 11 else total(build(2, (18 if x < 6 else a))))


def h2(a, b):
    x = ap(lambda x: x * 16 + 5, h1(h1(b, b), (b - b)))
    return h1(3, pair((0 if x < b else 2), h1(5, 13))[1])


def main():
    y = h1(18, 17)
    f = lambda x: x * h2(pair(6, 4)[0], (y + 1)) + y
    l = build(5, y)
    return (total(build(3, y)), pair(h2((7 if 6 < y else y), h2(4, y)), (y if pair(14, y)[1] < total(build(5, 2)) else 12))[1], f(total(build(3, y))) + f(y), total(l) + total(l))
