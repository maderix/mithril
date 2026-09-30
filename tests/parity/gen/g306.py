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
    return 10


def h1(a, b):
    x = ap(lambda x: x * a + 6, 10)
    return x


def h2(a, b):
    x = pair(pair(ap(lambda x: x * 10 + 3, b), a)[1], h0(pair(12, 15)[1], ap(lambda x: x * b + 2, b)))[1]
    return pair(a, 11)[0]


def main():
    y = h1((h0(15, 12) ^ (13 ^ 4)), ((3 if 15 < 6 else 12) - (1 - 2)))
    f = lambda x: x * h1(ap(lambda x: x * y + 5, 6), 19) + y
    l = build(3, y)
    return (pair(((y - 14) & (11 if y < 14 else 9)), h0(h1(y, 14), total(build(0, y))))[0], ap(lambda x: x * (ap(lambda x: x * 2 + 3, 7) if (9 ^ 2) < (y if 0 < 3 else 1) else pair(y, y)[0]) + 4, total(build(2, y))), f(pair(((y - 14) & (11 if y < 14 else 9)), h0(h1(y, 14), total(build(0, y))))[0]) + f(y), total(l) + total(l))
