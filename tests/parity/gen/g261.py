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
    x = (pair(total(build(5, 3)), (a ^ 16))[1] if 17 < (a - b) else b)
    return a


def h1(a, b):
    x = (((a if b < 15 else 7) if total(build(3, b)) < total(build(6, 11)) else ap(lambda x: x * 7 + 7, 3)) if total(build(4, pair(6, a)[1])) < a else total(build(0, (19 ^ b))))
    return (b * total(build(1, b)))


def h2(a, b):
    x = ap(lambda x: x * 18 + 0, ((10 - 19) & 15))
    return h0(pair((9 & 5), h1(x, x))[0], b)


def main():
    y = 19
    f = lambda x: x * ap(lambda x: x * total(build(4, y)) + 1, pair(y, 15)[1]) + y
    l = build(4, y)
    return (8, (y if h0(18, y) < y else total(build(3, y))), f(8) + f(y), total(l) + total(l))
