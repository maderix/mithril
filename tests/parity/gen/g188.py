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
    x = (pair((7 - 14), 6)[0] if b < (total(build(1, 10)) if ap(lambda x: x * 16 + 1, 9) < ap(lambda x: x * b + 7, 6) else total(build(4, 2))) else pair(b, b)[0])
    return pair(pair(b, 11)[0], (pair(15, x)[0] + 3))[0]


def h1(a, b):
    x = pair(pair(12, (a * a))[1], total(build(5, (6 if b < 11 else a))))[0]
    return a


def h2(a, b):
    x = ap(lambda x: x * (7 - total(build(6, a))) + 1, 14)
    return ((x if (7 if 2 < 1 else 4) < 2 else a) if b < h1((x if 19 < b else 15), (x if 16 < 4 else b)) else a)


def main():
    y = ap(lambda x: x * total(build(6, h0(0, 0))) + 2, total(build(3, h0(5, 6))))
    f = lambda x: x * (h2(19, y) & 12) + y
    l = build(5, y)
    return (y, h1(h2(total(build(0, y)), 9), 4), f(y) + f(y), total(l) + total(l))
