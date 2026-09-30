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
    x = total(build(6, pair((7 - 1), total(build(2, 10)))[0]))
    return 0


def h1(a, b):
    x = 14
    return pair(ap(lambda x: x * pair(a, x)[0] + 6, total(build(4, 9))), total(build(5, ap(lambda x: x * 1 + 4, b))))[1]


def h2(a, b):
    x = ap(lambda x: x * 10 + 0, total(build(4, (16 - b))))
    return (pair((14 & 9), (9 if b < x else 14))[0] if (pair(x, 11)[0] if b < (b if x < 5 else 17) else b) < (b ^ pair(18, 3)[1]) else (b if (13 if 19 < 17 else 11) < (x if 14 < 2 else b) else b))


def main():
    y = pair(h2(15, ap(lambda x: x * 8 + 8, 1)), h2(1, ap(lambda x: x * 10 + 3, 4)))[0]
    f = lambda x: x * 12 + y
    l = build(4, y)
    return (pair(pair(5, h1(y, 1))[1], (y if 16 < (y if y < y else 11) else (y + y)))[0], y, f(pair(pair(5, h1(y, 1))[1], (y if 16 < (y if y < y else 11) else (y + y)))[0]) + f(y), total(l) + total(l))
