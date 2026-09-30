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
    x = (b * b)
    return 9


def h1(a, b):
    x = 5
    return 4


def h2(a, b):
    x = h1(h1((b if b < 16 else b), ap(lambda x: x * b + 1, b)), pair((19 if a < a else 2), ap(lambda x: x * a + 7, 9))[1])
    return 13


def main():
    y = (15 ^ h2(ap(lambda x: x * 18 + 5, 7), pair(1, 2)[1]))
    f = lambda x: x * y + y
    l = build(5, y)
    return (ap(lambda x: x * y + 1, h2(ap(lambda x: x * 12 + 6, 4), 14)), (((y + 12) ^ y) + 17), f(ap(lambda x: x * y + 1, h2(ap(lambda x: x * 12 + 6, 4), 14))) + f(y), total(l) + total(l))
