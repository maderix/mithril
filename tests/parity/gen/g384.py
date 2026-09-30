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
    x = total(build(3, (15 if 12 < ap(lambda x: x * b + 4, b) else a)))
    return a


def h1(a, b):
    x = total(build(3, 0))
    return ((total(build(1, x)) ^ (a if 12 < 16 else 13)) if h0(pair(b, a)[0], 5) < (h0(b, 4) - (x - b)) else b)


def h2(a, b):
    x = (pair(pair(b, 5)[1], b)[1] if (total(build(1, 8)) * b) < total(build(3, total(build(4, b)))) else 3)
    return 12


def main():
    y = pair((total(build(6, 3)) * total(build(4, 0))), 6)[1]
    f = lambda x: x * h1(pair(y, y)[0], 0) + y
    l = build(2, y)
    return (ap(lambda x: x * (1 & pair(y, y)[0]) + 1, (ap(lambda x: x * 16 + 6, 3) if (5 + y) < total(build(2, 7)) else total(build(6, 14)))), (ap(lambda x: x * ap(lambda x: x * y + 1, y) + 1, pair(y, y)[0]) if (11 if ap(lambda x: x * 16 + 6, 19) < total(build(4, 6)) else pair(y, y)[0]) < h0(y, ap(lambda x: x * 3 + 4, y)) else h2((y if 9 < 12 else y), (y & 8))), f(ap(lambda x: x * (1 & pair(y, y)[0]) + 1, (ap(lambda x: x * 16 + 6, 3) if (5 + y) < total(build(2, 7)) else total(build(6, 14))))) + f(y), total(l) + total(l))
