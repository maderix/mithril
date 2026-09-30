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
    x = ap(lambda x: x * 15 + 6, 1)
    return ap(lambda x: x * 19 + 0, (a if 12 < a else (4 if 14 < 18 else x)))


def h1(a, b):
    x = total(build(6, ((16 if 14 < b else b) ^ ap(lambda x: x * b + 2, 7))))
    return 18


def h2(a, b):
    x = h1(a, 11)
    return h1(b, b)


def main():
    y = (ap(lambda x: x * (19 + 19) + 5, ap(lambda x: x * 11 + 3, 5)) if 18 < 1 else ((18 if 3 < 8 else 14) - (15 - 1)))
    f = lambda x: x * pair(2, pair(3, 11)[0])[1] + y
    l = build(5, y)
    return (h2(2, h2(y, h2(y, y))), ap(lambda x: x * 11 + 7, y), f(h2(2, h2(y, h2(y, y)))) + f(y), total(l) + total(l))
