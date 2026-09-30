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
    x = total(build(3, 12))
    return 5


def h1(a, b):
    x = pair(pair(18, total(build(0, 0)))[1], ((18 if b < 14 else a) if (a - 10) < (2 & 12) else ap(lambda x: x * a + 7, a)))[1]
    return pair(19, ((a if 18 < a else x) * ap(lambda x: x * a + 8, b)))[1]


def h2(a, b):
    x = ap(lambda x: x * (total(build(3, a)) if (2 if 7 < 6 else a) < 3 else pair(b, b)[1]) + 6, total(build(6, total(build(5, b)))))
    return a


def main():
    y = ap(lambda x: x * 17 + 4, h0(19, (5 - 16)))
    f = lambda x: x * ap(lambda x: x * (18 + y) + 0, (11 ^ 1)) + y
    l = build(4, y)
    return (total(build(5, 1)), total(build(5, 5)), f(total(build(5, 1))) + f(y), total(l) + total(l))
