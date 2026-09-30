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
    return (b - 19)


def h1(a, b):
    x = a
    return h0(total(build(0, h0(15, 15))), h0(total(build(1, 17)), (13 if b < x else b)))


def h2(a, b):
    x = total(build(5, 5))
    return total(build(4, h0(ap(lambda x: x * b + 6, a), (19 if b < 0 else b))))


def main():
    y = ap(lambda x: x * ap(lambda x: x * (3 + 5) + 0, pair(13, 6)[1]) + 4, (total(build(4, 17)) ^ (12 ^ 3)))
    f = lambda x: x * ((y * y) if (1 if y < y else y) < ap(lambda x: x * y + 5, y) else (y if 13 < 4 else 6)) + y
    l = build(5, y)
    return (total(build(0, h2(pair(y, y)[0], ap(lambda x: x * 15 + 1, y)))), (h2((y if y < y else y), h2(3, y)) & (18 & 12)), f(total(build(0, h2(pair(y, y)[0], ap(lambda x: x * 15 + 1, y))))) + f(y), total(l) + total(l))
