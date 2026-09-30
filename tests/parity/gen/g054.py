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
    x = pair(6, a)[1]
    return pair(10, 1)[0]


def h1(a, b):
    x = total(build(0, ap(lambda x: x * ap(lambda x: x * b + 5, b) + 8, 13)))
    return (h0((b if b < x else a), ap(lambda x: x * 15 + 2, 15)) - h0(1, ap(lambda x: x * 15 + 1, 7)))


def h2(a, b):
    x = total(build(3, 7))
    return (6 + (x * (0 ^ x)))


def main():
    y = 19
    f = lambda x: x * total(build(5, (y & 7))) + y
    l = build(5, y)
    return (h2(pair((y if 0 < y else y), 9)[1], pair(y, y)[0]), ap(lambda x: x * total(build(0, 10)) + 5, h1((4 & y), y)), f(h2(pair((y if 0 < y else y), 9)[1], pair(y, y)[0])) + f(y), total(l) + total(l))
