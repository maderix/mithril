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
    x = 0
    return 10


def h1(a, b):
    x = (16 + pair(ap(lambda x: x * 19 + 3, b), pair(0, 13)[1])[0])
    return 11


def h2(a, b):
    x = pair(ap(lambda x: x * h1(b, b) + 7, h0(14, 8)), pair(pair(19, b)[1], total(build(4, a)))[0])[1]
    return pair(((x if 15 < a else 4) - b), x)[1]


def main():
    y = 6
    f = lambda x: x * y + y
    l = build(4, y)
    return (y, ap(lambda x: x * ap(lambda x: x * (y ^ 7) + 8, pair(y, y)[1]) + 6, y), f(y) + f(y), total(l) + total(l))
