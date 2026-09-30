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
    x = ap(lambda x: x * a + 2, pair(pair(0, b)[0], 17)[1])
    return 5


def h1(a, b):
    x = ap(lambda x: x * (18 * total(build(1, 15))) + 0, (ap(lambda x: x * 12 + 0, 15) if ap(lambda x: x * 8 + 3, a) < ap(lambda x: x * 3 + 3, 6) else total(build(5, b))))
    return x


def h2(a, b):
    x = (total(build(0, total(build(4, 1)))) & 7)
    return total(build(4, pair((4 ^ 0), a)[0]))


def main():
    y = 15
    f = lambda x: x * ap(lambda x: x * (y & y) + 0, (9 & 2)) + y
    l = build(5, y)
    return (y, (2 & 18), f(y) + f(y), total(l) + total(l))
