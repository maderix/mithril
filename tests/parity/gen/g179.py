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
    x = (a & 17)
    return ap(lambda x: x * 17 + 2, b)


def h1(a, b):
    x = ((pair(b, a)[0] if ap(lambda x: x * a + 2, a) < (b if 17 < 15 else 15) else b) - a)
    return h0((2 + pair(1, 10)[1]), total(build(5, (x * a))))


def h2(a, b):
    x = b
    return h1(h1(b, ap(lambda x: x * x + 7, 5)), 12)


def main():
    y = 3
    f = lambda x: x * pair(h1(y, 16), (4 if 18 < y else 1))[0] + y
    l = build(6, y)
    return (ap(lambda x: x * 11 + 6, pair(y, h0(8, 14))[0]), ap(lambda x: x * total(build(3, y)) + 3, h0(ap(lambda x: x * y + 8, y), 0)), f(ap(lambda x: x * 11 + 6, pair(y, h0(8, 14))[0])) + f(y), total(l) + total(l))
