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
    x = 12
    return 16


def h1(a, b):
    x = (17 if a < ap(lambda x: x * 15 + 1, 4) else a)
    return pair(b, 6)[1]


def h2(a, b):
    x = pair(10, ap(lambda x: x * ap(lambda x: x * b + 6, b) + 1, (b & 0)))[0]
    return ((x + pair(b, 13)[1]) & pair(a, a)[0])


def main():
    y = 17
    f = lambda x: x * 12 + y
    l = build(2, y)
    return ((ap(lambda x: x * (y if y < y else 18) + 8, (12 if y < y else 6)) * h1(h0(y, y), h2(12, y))), ap(lambda x: x * pair(y, h2(1, 17))[0] + 8, h2(y, y)), f((ap(lambda x: x * (y if y < y else 18) + 8, (12 if y < y else 6)) * h1(h0(y, y), h2(12, y)))) + f(y), total(l) + total(l))
