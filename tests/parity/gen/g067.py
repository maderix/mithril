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
    x = ap(lambda x: x * b + 2, total(build(5, 3)))
    return (a + 10)


def h1(a, b):
    x = (a if (ap(lambda x: x * 9 + 4, 11) & ap(lambda x: x * 1 + 0, a)) < h0(h0(14, b), (a - a)) else (b if ap(lambda x: x * b + 6, 15) < (16 ^ a) else total(build(1, b))))
    return pair(((18 & b) if (3 + 16) < 15 else x), h0((b if 15 < a else a), total(build(6, b))))[0]


def h2(a, b):
    x = (h0(ap(lambda x: x * 3 + 5, a), ap(lambda x: x * b + 4, b)) * h1(b, h1(b, b)))
    return ap(lambda x: x * pair(pair(9, b)[1], x)[1] + 8, a)


def main():
    y = total(build(5, pair((9 if 7 < 0 else 19), (19 * 0))[0]))
    f = lambda x: x * (h0(17, y) * h0(8, 8)) + y
    l = build(1, y)
    return (total(build(2, total(build(2, 3)))), ((y ^ (3 if 0 < 12 else y)) - 9), f(total(build(2, total(build(2, 3))))) + f(y), total(l) + total(l))
