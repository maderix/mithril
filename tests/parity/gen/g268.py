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
    x = total(build(3, total(build(4, total(build(2, b))))))
    return pair(a, ((6 + 0) if pair(19, 0)[1] < ap(lambda x: x * 0 + 4, x) else pair(a, 18)[0]))[1]


def h1(a, b):
    x = ap(lambda x: x * (pair(8, a)[0] if pair(b, 12)[0] < (a if a < 16 else b) else h0(1, b)) + 1, pair(ap(lambda x: x * b + 1, 19), pair(17, 5)[1])[1])
    return 16


def h2(a, b):
    x = total(build(4, pair((a if 11 < 18 else 19), pair(a, 9)[1])[0]))
    return (19 & a)


def main():
    y = pair((total(build(0, 7)) - (16 - 11)), (6 + total(build(1, 5))))[0]
    f = lambda x: x * total(build(4, pair(y, y)[0])) + y
    l = build(5, y)
    return (pair(4, pair((1 if y < 13 else y), h2(y, y))[0])[1], h1(total(build(6, (19 if 3 < 6 else y))), ap(lambda x: x * pair(4, 1)[0] + 4, 5)), f(pair(4, pair((1 if y < 13 else y), h2(y, y))[0])[1]) + f(y), total(l) + total(l))
