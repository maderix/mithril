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
    x = (b if pair(ap(lambda x: x * 2 + 6, 0), 16)[1] < total(build(0, ap(lambda x: x * b + 2, 12))) else 11)
    return (pair((2 if 10 < b else b), (2 & 3))[1] + total(build(6, pair(a, b)[0])))


def h1(a, b):
    x = b
    return pair(total(build(0, (b + b))), b)[1]


def h2(a, b):
    x = (total(build(6, (a - 8))) if pair((4 - a), (18 - a))[1] < pair(pair(b, 16)[1], (b if 7 < b else 16))[1] else pair(18, pair(b, a)[0])[0])
    return 6


def main():
    y = 12
    f = lambda x: x * total(build(4, (y if y < y else 1))) + y
    l = build(2, y)
    return (13, ((h0(18, 9) if 16 < (10 - y) else y) * 4), f(13) + f(y), total(l) + total(l))
