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
    x = a
    return 4


def h1(a, b):
    x = ap(lambda x: x * (total(build(0, a)) if (5 + 0) < (b ^ 10) else (a if 17 < a else 13)) + 2, h0(ap(lambda x: x * 2 + 7, 19), (a if a < 15 else 13)))
    return ap(lambda x: x * b + 2, 17)


def h2(a, b):
    x = ap(lambda x: x * pair(b, h1(14, a))[0] + 3, ap(lambda x: x * (a & 0) + 5, pair(a, 3)[0]))
    return (pair(total(build(3, x)), a)[1] ^ 17)


def main():
    y = total(build(3, total(build(5, h0(0, 0)))))
    f = lambda x: x * 5 + y
    l = build(2, y)
    return (y, h1(pair(pair(18, 0)[0], total(build(3, 8)))[0], 19), f(y) + f(y), total(l) + total(l))
