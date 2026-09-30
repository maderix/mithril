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
    x = total(build(1, ap(lambda x: x * b + 5, pair(a, a)[0])))
    return 3


def h1(a, b):
    x = (h0((5 - 16), pair(8, b)[0]) if 0 < ap(lambda x: x * ap(lambda x: x * 3 + 3, a) + 8, b) else (ap(lambda x: x * a + 4, b) if (a if 16 < a else a) < 4 else pair(9, a)[1]))
    return (a ^ a)


def h2(a, b):
    x = (b & ap(lambda x: x * 3 + 0, pair(b, 17)[0]))
    return (x if 12 < total(build(4, (a & 12))) else total(build(2, pair(b, b)[0])))


def main():
    y = total(build(5, 6))
    f = lambda x: x * y + y
    l = build(2, y)
    return (y, ap(lambda x: x * ((y & y) & 9) + 6, y), f(y) + f(y), total(l) + total(l))
