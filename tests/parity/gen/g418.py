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
    x = pair(13, 18)[1]
    return pair(pair(16, (13 if 4 < 8 else b))[0], (total(build(0, x)) if (x if a < 4 else 19) < 2 else (7 if x < a else 11)))[1]


def h1(a, b):
    x = (((16 if a < a else 13) if 12 < ap(lambda x: x * a + 0, 19) else (19 * b)) * (h0(b, b) if a < (b & 19) else (8 if a < 4 else 12)))
    return (pair(pair(9, a)[1], pair(15, 10)[0])[0] - 3)


def h2(a, b):
    x = a
    return ((h0(18, 0) * pair(b, b)[1]) & total(build(0, h0(x, 13))))


def main():
    y = total(build(0, 8))
    f = lambda x: x * 19 + y
    l = build(0, y)
    return (ap(lambda x: x * y + 3, ap(lambda x: x * (y if y < 5 else 4) + 5, 8)), ap(lambda x: x * 8 + 4, 13), f(ap(lambda x: x * y + 3, ap(lambda x: x * (y if y < 5 else 4) + 5, 8))) + f(y), total(l) + total(l))
