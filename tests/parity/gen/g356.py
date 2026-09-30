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
    x = pair(ap(lambda x: x * a + 8, ap(lambda x: x * 6 + 4, a)), pair(total(build(5, 5)), pair(a, b)[1])[0])[0]
    return (a if ((6 ^ a) - a) < total(build(3, 11)) else 19)


def h1(a, b):
    x = (h0(14, (7 if 1 < a else b)) if 3 < pair((b & a), h0(b, a))[1] else (6 if ap(lambda x: x * b + 0, b) < pair(b, 19)[1] else total(build(5, 17))))
    return ap(lambda x: x * (ap(lambda x: x * x + 0, 18) * 8) + 6, (13 - 15))


def h2(a, b):
    x = total(build(2, (pair(b, 16)[1] * ap(lambda x: x * 14 + 8, b))))
    return b


def main():
    y = 9
    f = lambda x: x * y + y
    l = build(6, y)
    return (ap(lambda x: x * ap(lambda x: x * 5 + 1, pair(6, y)[0]) + 7, 8), (y if ap(lambda x: x * pair(4, y)[0] + 6, ap(lambda x: x * y + 5, y)) < (pair(4, y)[1] if 14 < (y if y < 6 else 2) else 10) else (y + (y * y))), f(ap(lambda x: x * ap(lambda x: x * 5 + 1, pair(6, y)[0]) + 7, 8)) + f(y), total(l) + total(l))
