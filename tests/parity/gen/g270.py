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
    x = (pair(11, 5)[0] if total(build(6, (b if 11 < a else a))) < ((a if 12 < a else a) if (15 if a < 0 else 1) < (1 * 5) else total(build(2, a))) else b)
    return 0


def h1(a, b):
    x = total(build(6, 17))
    return 13


def h2(a, b):
    x = ap(lambda x: x * b + 8, 17)
    return pair(total(build(1, b)), (pair(11, 8)[0] - pair(a, 9)[0]))[0]


def main():
    y = total(build(3, (17 if 13 < h2(17, 4) else 12)))
    f = lambda x: x * (pair(8, y)[0] if total(build(4, y)) < h0(2, 13) else y) + y
    l = build(4, y)
    return (total(build(1, (13 + (y if 7 < y else 8)))), pair((ap(lambda x: x * 19 + 6, y) if 17 < y else h2(y, 13)), h2(h1(y, 13), 2))[1], f(total(build(1, (13 + (y if 7 < y else 8))))) + f(y), total(l) + total(l))
