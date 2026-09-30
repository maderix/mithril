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
    x = total(build(3, (pair(2, a)[1] if 18 < (a if a < 4 else 12) else ap(lambda x: x * 13 + 4, b))))
    return pair(5, pair(total(build(6, a)), ap(lambda x: x * 11 + 0, b))[0])[1]


def h1(a, b):
    x = h0((h0(a, 3) - ap(lambda x: x * a + 7, a)), h0(ap(lambda x: x * 17 + 8, 7), a))
    return a


def h2(a, b):
    x = a
    return total(build(4, 13))


def main():
    y = total(build(3, (pair(7, 11)[0] if 11 < ap(lambda x: x * 13 + 8, 16) else h2(2, 12))))
    f = lambda x: x * (y - pair(15, y)[1]) + y
    l = build(4, y)
    return (y, pair(y, 10)[0], f(y) + f(y), total(l) + total(l))
