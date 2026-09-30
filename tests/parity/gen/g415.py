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
    x = pair((5 * total(build(2, b))), 2)[0]
    return total(build(3, (ap(lambda x: x * x + 4, a) if 3 < (18 & a) else 2)))


def h1(a, b):
    x = 17
    return a


def h2(a, b):
    x = pair((ap(lambda x: x * a + 4, b) if total(build(4, a)) < 17 else (0 + 0)), pair(h1(3, a), ap(lambda x: x * 7 + 4, b))[0])[1]
    return ap(lambda x: x * h0(h1(x, x), (16 if 11 < a else b)) + 5, h1((a - 8), (4 - b)))


def main():
    y = h0((h1(14, 6) if 12 < pair(9, 10)[1] else pair(3, 11)[0]), 17)
    f = lambda x: x * 0 + y
    l = build(1, y)
    return (y, total(build(1, y)), f(y) + f(y), total(l) + total(l))
