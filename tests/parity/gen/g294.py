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
    x = pair((ap(lambda x: x * 16 + 2, 16) & total(build(1, 18))), pair(b, 13)[0])[0]
    return (19 if b < 7 else (pair(7, b)[0] - (x if x < 16 else 16)))


def h1(a, b):
    x = 12
    return h0(total(build(5, h0(x, 0))), 10)


def h2(a, b):
    x = 10
    return (h0((17 & 11), (15 if x < 17 else 16)) if h1(ap(lambda x: x * x + 8, a), (3 if 18 < x else 6)) < h1(8, (2 if b < x else 2)) else x)


def main():
    y = 17
    f = lambda x: x * ap(lambda x: x * total(build(4, 15)) + 4, pair(7, y)[1]) + y
    l = build(2, y)
    return (total(build(3, 8)), 1, f(total(build(3, 8))) + f(y), total(l) + total(l))
