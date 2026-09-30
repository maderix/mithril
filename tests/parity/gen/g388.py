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
    x = (ap(lambda x: x * (11 if 0 < 19 else 1) + 8, ap(lambda x: x * 10 + 0, b)) * (13 if 14 < (a + 14) else 1))
    return pair(9, (3 - (a & a)))[0]


def h1(a, b):
    x = pair(total(build(5, b)), pair(h0(1, b), total(build(2, b)))[1])[1]
    return 12


def h2(a, b):
    x = h1(total(build(6, total(build(0, 19)))), 8)
    return h0((pair(0, 3)[1] - (a + x)), 7)


def main():
    y = 0
    f = lambda x: x * ap(lambda x: x * (y if y < y else 12) + 5, (17 & y)) + y
    l = build(3, y)
    return (total(build(3, total(build(2, pair(y, 6)[1])))), h2(y, 10), f(total(build(3, total(build(2, pair(y, 6)[1]))))) + f(y), total(l) + total(l))
