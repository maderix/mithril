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
    x = pair(4, total(build(4, total(build(1, 13)))))[0]
    return ap(lambda x: x * 5 + 7, pair(total(build(4, 19)), b)[1])


def h1(a, b):
    x = ap(lambda x: x * (total(build(1, 1)) & (a if a < 12 else 11)) + 5, ap(lambda x: x * h0(a, 0) + 6, (0 if 9 < 18 else 8)))
    return (18 if x < b else x)


def h2(a, b):
    x = total(build(1, 9))
    return (h1(pair(b, 0)[0], x) if 16 < h1(a, ap(lambda x: x * a + 1, 9)) else 3)


def main():
    y = total(build(3, (h0(15, 7) if 16 < h0(6, 13) else ap(lambda x: x * 8 + 4, 2))))
    f = lambda x: x * ap(lambda x: x * pair(y, 15)[0] + 7, h2(y, 19)) + y
    l = build(2, y)
    return (y, total(build(4, y)), f(y) + f(y), total(l) + total(l))
