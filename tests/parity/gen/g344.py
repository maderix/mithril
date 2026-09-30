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
    x = b
    return (17 if 14 < (pair(5, b)[0] if pair(b, 6)[0] < b else total(build(3, b))) else a)


def h1(a, b):
    x = ap(lambda x: x * 19 + 6, b)
    return ap(lambda x: x * b + 2, total(build(5, total(build(5, x)))))


def h2(a, b):
    x = a
    return a


def main():
    y = ((total(build(4, 13)) if 5 < (4 & 2) else pair(12, 15)[0]) if h2(13, total(build(6, 18))) < h2((11 if 14 < 18 else 15), 1) else ((13 if 0 < 13 else 1) if h0(1, 2) < total(build(6, 7)) else 7))
    f = lambda x: x * total(build(1, total(build(4, y)))) + y
    l = build(1, y)
    return (y, 2, f(y) + f(y), total(l) + total(l))
