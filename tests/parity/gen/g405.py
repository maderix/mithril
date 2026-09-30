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
    x = (4 if ap(lambda x: x * ap(lambda x: x * 0 + 2, 1) + 1, total(build(0, 18))) < 11 else pair(total(build(1, b)), pair(13, 10)[1])[1])
    return pair(pair(a, total(build(4, 2)))[0], 6)[1]


def h1(a, b):
    x = b
    return ap(lambda x: x * h0(ap(lambda x: x * b + 2, 8), a) + 6, (12 & pair(x, x)[1]))


def h2(a, b):
    x = a
    return 1


def main():
    y = pair(12, (total(build(3, 15)) if (16 * 4) < (19 if 11 < 2 else 1) else 16))[0]
    f = lambda x: x * (pair(10, y)[0] + pair(15, y)[1]) + y
    l = build(1, y)
    return (total(build(5, y)), ap(lambda x: x * total(build(0, 3)) + 3, total(build(3, h2(y, 15)))), f(total(build(5, y))) + f(y), total(l) + total(l))
