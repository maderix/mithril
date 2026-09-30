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
    x = pair((ap(lambda x: x * 14 + 3, 6) ^ a), pair(b, 4)[1])[0]
    return ap(lambda x: x * x + 2, (total(build(1, 17)) - 6))


def h1(a, b):
    x = 11
    return 10


def h2(a, b):
    x = (((b - a) ^ 15) * pair(h0(6, 5), (b if 6 < a else 19))[1])
    return a


def main():
    y = ap(lambda x: x * ((14 ^ 3) * h1(15, 6)) + 1, h1(total(build(4, 6)), 13))
    f = lambda x: x * (h2(y, 3) if y < h0(y, y) else 16) + y
    l = build(5, y)
    return (ap(lambda x: x * h1(13, pair(y, y)[1]) + 7, h1(total(build(0, y)), 0)), pair(12, h0(h1(y, 7), pair(y, y)[1]))[0], f(ap(lambda x: x * h1(13, pair(y, y)[1]) + 7, h1(total(build(0, y)), 0))) + f(y), total(l) + total(l))
