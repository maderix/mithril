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
    x = 6
    return (total(build(5, b)) * ap(lambda x: x * a + 2, ap(lambda x: x * a + 6, b)))


def h1(a, b):
    x = h0(total(build(1, 17)), (2 if h0(b, 4) < a else ap(lambda x: x * a + 8, 0)))
    return (pair(total(build(4, x)), ap(lambda x: x * 7 + 1, b))[0] if (h0(11, x) if pair(17, 18)[1] < ap(lambda x: x * 18 + 4, a) else (a ^ b)) < (19 if (1 if 4 < a else a) < h0(x, 13) else ap(lambda x: x * 1 + 3, a)) else b)


def h2(a, b):
    x = ap(lambda x: x * b + 2, pair((a ^ b), 9)[0])
    return ap(lambda x: x * pair(x, ap(lambda x: x * a + 0, x))[0] + 5, total(build(4, pair(b, 5)[0])))


def main():
    y = pair(pair(total(build(4, 11)), 19)[1], 2)[1]
    f = lambda x: x * (total(build(0, y)) if ap(lambda x: x * y + 1, 15) < 13 else (17 ^ 11)) + y
    l = build(0, y)
    return (1, h1((total(build(3, y)) ^ 15), ap(lambda x: x * ap(lambda x: x * 9 + 8, y) + 2, h2(8, 13))), f(1) + f(y), total(l) + total(l))
