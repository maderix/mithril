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
    x = 2
    return 5


def h1(a, b):
    x = b
    return (ap(lambda x: x * total(build(5, 14)) + 3, x) + ap(lambda x: x * x + 0, total(build(6, b))))


def h2(a, b):
    x = h1(h0(a, pair(b, a)[1]), pair(h1(a, b), pair(4, 2)[1])[1])
    return 11


def main():
    y = (h2((14 & 6), ap(lambda x: x * 6 + 0, 17)) * ((1 - 13) if 18 < (9 if 17 < 1 else 4) else (8 + 3)))
    f = lambda x: x * (ap(lambda x: x * 2 + 7, 1) & ap(lambda x: x * y + 2, 2)) + y
    l = build(0, y)
    return (11, total(build(6, pair(ap(lambda x: x * y + 1, 19), 10)[0])), f(11) + f(y), total(l) + total(l))
