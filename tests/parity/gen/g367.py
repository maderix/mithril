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
    x = pair(a, (pair(15, a)[1] * (3 if b < 13 else b)))[0]
    return pair(ap(lambda x: x * ap(lambda x: x * b + 0, x) + 0, total(build(4, a))), (ap(lambda x: x * b + 3, 7) if ap(lambda x: x * 8 + 8, a) < pair(7, a)[1] else pair(11, b)[1]))[0]


def h1(a, b):
    x = 0
    return (pair(b, pair(11, 10)[0])[1] & x)


def h2(a, b):
    x = (pair((a if b < 16 else a), ap(lambda x: x * a + 6, 8))[0] if h1((b if a < 18 else b), (b if 13 < a else b)) < a else a)
    return (h1((9 ^ 7), (b if a < 7 else x)) ^ total(build(4, h1(b, 17))))


def main():
    y = pair((7 * total(build(6, 4))), pair(h2(16, 17), pair(10, 1)[0])[0])[0]
    f = lambda x: x * 3 + y
    l = build(5, y)
    return ((ap(lambda x: x * (9 + y) + 3, (y if y < y else 6)) - pair((17 if 4 < y else 9), total(build(5, 17)))[0]), total(build(4, ap(lambda x: x * total(build(1, 16)) + 8, 10))), f((ap(lambda x: x * (9 + y) + 3, (y if y < y else 6)) - pair((17 if 4 < y else 9), total(build(5, 17)))[0])) + f(y), total(l) + total(l))
