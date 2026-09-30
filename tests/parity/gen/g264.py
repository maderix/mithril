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
    x = (((b & a) if a < ap(lambda x: x * a + 8, 9) else (16 & 5)) + ap(lambda x: x * 1 + 0, 13))
    return 12


def h1(a, b):
    x = h0(total(build(0, 13)), ap(lambda x: x * total(build(5, 7)) + 8, a))
    return (a ^ 3)


def h2(a, b):
    x = (ap(lambda x: x * ap(lambda x: x * a + 3, b) + 3, (9 if 18 < a else 12)) if 2 < total(build(4, 11)) else 11)
    return ((4 ^ (b * 0)) - total(build(2, 3)))


def main():
    y = 13
    f = lambda x: x * 0 + y
    l = build(0, y)
    return (16, y, f(16) + f(y), total(l) + total(l))
