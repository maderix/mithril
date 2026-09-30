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
    x = 8
    return ap(lambda x: x * b + 6, 5)


def h1(a, b):
    x = b
    return pair(h0(h0(12, 12), pair(a, b)[1]), pair(b, (4 ^ 6))[1])[0]


def h2(a, b):
    x = b
    return 3


def main():
    y = 16
    f = lambda x: x * total(build(3, total(build(3, 6)))) + y
    l = build(0, y)
    return (ap(lambda x: x * (y + y) + 2, pair(pair(y, y)[0], pair(y, 17)[0])[0]), ap(lambda x: x * total(build(0, (11 if 12 < y else 12))) + 1, h0(ap(lambda x: x * 17 + 1, 11), (y & y))), f(ap(lambda x: x * (y + y) + 2, pair(pair(y, y)[0], pair(y, 17)[0])[0])) + f(y), total(l) + total(l))
