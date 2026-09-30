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
    return (((10 + 8) - pair(2, 4)[1]) if (14 ^ total(build(5, b))) < total(build(6, b)) else (pair(a, 8)[1] if total(build(6, b)) < 13 else a))


def h1(a, b):
    x = pair(b, 13)[0]
    return x


def h2(a, b):
    x = ap(lambda x: x * total(build(2, 19)) + 3, pair(total(build(4, 13)), 18)[0])
    return b


def main():
    y = 19
    f = lambda x: x * (ap(lambda x: x * 8 + 2, 8) + (y * 10)) + y
    l = build(1, y)
    return (pair(pair(3, pair(7, y)[0])[0], (pair(4, y)[0] & ap(lambda x: x * y + 3, y)))[0], y, f(pair(pair(3, pair(7, y)[0])[0], (pair(4, y)[0] & ap(lambda x: x * y + 3, y)))[0]) + f(y), total(l) + total(l))
