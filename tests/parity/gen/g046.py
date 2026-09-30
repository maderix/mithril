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
    x = 9
    return (((x if 2 < x else 13) + 1) + ap(lambda x: x * (x & a) + 0, (b if 8 < 5 else x)))


def h1(a, b):
    x = a
    return ((pair(12, b)[0] ^ pair(18, 15)[0]) & ap(lambda x: x * pair(18, 1)[0] + 1, pair(a, a)[0]))


def h2(a, b):
    x = 19
    return total(build(4, ap(lambda x: x * h1(1, 8) + 5, (15 if b < b else a))))


def main():
    y = ((h0(1, 4) & ap(lambda x: x * 6 + 8, 10)) * (19 ^ 16))
    f = lambda x: x * y + y
    l = build(2, y)
    return ((y - ap(lambda x: x * 3 + 4, (2 + 14))), y, f((y - ap(lambda x: x * 3 + 4, (2 + 14)))) + f(y), total(l) + total(l))
