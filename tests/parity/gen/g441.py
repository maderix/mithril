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
    x = ap(lambda x: x * 16 + 1, ap(lambda x: x * pair(b, a)[1] + 6, 16))
    return a


def h1(a, b):
    x = total(build(0, total(build(6, b))))
    return (h0(total(build(1, a)), pair(a, x)[0]) if h0(a, ap(lambda x: x * 15 + 3, 8)) < pair(17, (18 if 8 < a else b))[1] else b)


def h2(a, b):
    x = ap(lambda x: x * 11 + 3, ((b - 3) ^ pair(11, 4)[0]))
    return (((a - 17) if ap(lambda x: x * a + 7, 7) < ap(lambda x: x * b + 7, b) else ap(lambda x: x * 13 + 0, 6)) & pair((2 + b), b)[0])


def main():
    y = (h0(pair(16, 5)[1], 9) - 1)
    f = lambda x: x * h1(15, total(build(6, y))) + y
    l = build(4, y)
    return (15, total(build(0, (pair(y, y)[1] if total(build(0, 8)) < total(build(3, y)) else h2(4, 18)))), f(15) + f(y), total(l) + total(l))
