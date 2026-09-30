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
    x = total(build(5, 14))
    return (pair(19, x)[1] if 2 < total(build(3, total(build(0, 14)))) else (3 - pair(b, 10)[1]))


def h1(a, b):
    x = 8
    return 9


def h2(a, b):
    x = (a if total(build(1, pair(1, b)[1])) < a else ap(lambda x: x * total(build(6, 1)) + 4, h0(a, 14)))
    return ap(lambda x: x * h1((3 if 17 < x else 9), pair(11, 15)[0]) + 0, ((a if 4 < 5 else a) if h0(18, 14) < x else (x * 2)))


def main():
    y = total(build(5, (16 if (11 if 19 < 18 else 13) < total(build(4, 14)) else 4)))
    f = lambda x: x * ap(lambda x: x * pair(12, y)[1] + 7, 0) + y
    l = build(5, y)
    return (total(build(1, ap(lambda x: x * 13 + 1, (y if 10 < 10 else 14)))), ap(lambda x: x * (total(build(5, 1)) if (y & 4) < 7 else 6) + 8, (4 if pair(y, y)[1] < 10 else (y if y < y else 11))), f(total(build(1, ap(lambda x: x * 13 + 1, (y if 10 < 10 else 14))))) + f(y), total(l) + total(l))
