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
    x = 16
    return (ap(lambda x: x * 5 + 7, x) if b < (2 & 11) else 7)


def h1(a, b):
    x = 13
    return x


def h2(a, b):
    x = h0(((2 + b) if ap(lambda x: x * b + 6, a) < total(build(6, 9)) else (b if a < 15 else 5)), (h0(8, 5) if (a & a) < 3 else pair(a, a)[1]))
    return h1(pair((x if b < b else 0), ap(lambda x: x * a + 1, a))[1], (a - pair(15, b)[1]))


def main():
    y = ap(lambda x: x * ((11 + 19) + 14) + 3, h2(total(build(5, 18)), total(build(3, 12))))
    f = lambda x: x * 6 + y
    l = build(2, y)
    return (total(build(3, y)), h1(h1((y if 6 < 13 else y), (y if 5 < 11 else 0)), ap(lambda x: x * pair(19, 14)[0] + 6, pair(y, y)[1])), f(total(build(3, y))) + f(y), total(l) + total(l))
