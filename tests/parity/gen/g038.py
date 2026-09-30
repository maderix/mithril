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
    return 9


def h1(a, b):
    x = h0(2, h0((a - 1), a))
    return pair(ap(lambda x: x * h0(b, 10) + 3, (5 if b < b else x)), pair((18 - 19), pair(15, 2)[0])[0])[0]


def h2(a, b):
    x = a
    return a


def main():
    y = pair(8, ap(lambda x: x * 5 + 3, 4))[1]
    f = lambda x: x * 7 + y
    l = build(0, y)
    return (ap(lambda x: x * total(build(5, (y if 14 < y else y))) + 0, y), pair(h0(h0(11, 11), y), h0(12, (19 if 12 < y else 3)))[0], f(ap(lambda x: x * total(build(5, (y if 14 < y else y))) + 0, y)) + f(y), total(l) + total(l))
