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
    x = pair(a, pair(19, 2)[0])[1]
    return 18


def h1(a, b):
    x = pair(0, a)[0]
    return (6 * ap(lambda x: x * ap(lambda x: x * 11 + 0, a) + 6, 0))


def h2(a, b):
    x = a
    return (total(build(6, ap(lambda x: x * b + 6, a))) * (h0(x, a) if 7 < x else (5 + 11)))


def main():
    y = total(build(3, (total(build(5, 3)) * (4 + 17))))
    f = lambda x: x * h0(pair(y, y)[1], pair(4, y)[1]) + y
    l = build(1, y)
    return (total(build(1, ap(lambda x: x * (y ^ y) + 6, pair(16, 4)[0]))), y, f(total(build(1, ap(lambda x: x * (y ^ y) + 6, pair(16, 4)[0])))) + f(y), total(l) + total(l))
