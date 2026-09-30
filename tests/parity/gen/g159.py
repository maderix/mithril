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
    x = a
    return pair(((9 if x < 18 else b) if (x - x) < 0 else (x if 8 < 18 else a)), 17)[0]


def h1(a, b):
    x = ap(lambda x: x * b + 2, ap(lambda x: x * total(build(5, a)) + 4, (a if a < 8 else b)))
    return h0(pair((x * 1), (a if 18 < a else a))[0], x)


def h2(a, b):
    x = ap(lambda x: x * total(build(6, (19 - 11))) + 5, ((15 + 15) + h0(a, 16)))
    return pair(((b if 15 < 5 else b) if 8 < total(build(6, 8)) else total(build(3, 4))), h0(h0(0, 4), x))[1]


def main():
    y = (total(build(6, total(build(0, 11)))) ^ total(build(6, 6)))
    f = lambda x: x * ((10 * 18) if pair(y, 1)[0] < 5 else ap(lambda x: x * y + 3, y)) + y
    l = build(0, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
