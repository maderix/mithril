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
    return x


def h1(a, b):
    x = (((5 if b < a else 9) & h0(4, a)) ^ b)
    return 18


def h2(a, b):
    x = ((pair(2, 7)[0] if pair(5, b)[1] < a else (a if a < a else 14)) + ap(lambda x: x * 0 + 4, (b if 15 < a else b)))
    return a


def main():
    y = h1(18, (pair(7, 14)[0] - (13 if 6 < 16 else 2)))
    f = lambda x: x * ap(lambda x: x * pair(13, y)[0] + 0, y) + y
    l = build(1, y)
    return (pair(y, 12)[0], ((3 if y < ap(lambda x: x * y + 0, 3) else (18 if y < 11 else y)) - (y if (5 & y) < total(build(6, y)) else 4)), f(pair(y, 12)[0]) + f(y), total(l) + total(l))
