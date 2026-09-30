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
    x = 14
    return total(build(3, ap(lambda x: x * 12 + 7, ap(lambda x: x * 19 + 5, 18))))


def h1(a, b):
    x = 14
    return pair(h0(ap(lambda x: x * b + 0, a), h0(4, x)), h0(ap(lambda x: x * 7 + 1, b), pair(10, 15)[0]))[1]


def h2(a, b):
    x = (h0((a * a), (b + b)) if pair((5 if b < 10 else a), (a if 13 < a else b))[0] < pair(b, (9 if a < 3 else b))[1] else h1(10, 19))
    return ap(lambda x: x * ap(lambda x: x * h1(a, b) + 3, pair(a, x)[1]) + 5, h1((a if a < x else 8), b))


def main():
    y = 5
    f = lambda x: x * pair(h0(15, y), h1(y, 16))[0] + y
    l = build(0, y)
    return (ap(lambda x: x * ap(lambda x: x * pair(11, 5)[1] + 8, total(build(1, y))) + 0, 14), ap(lambda x: x * 12 + 8, total(build(4, ap(lambda x: x * y + 2, 15)))), f(ap(lambda x: x * ap(lambda x: x * pair(11, 5)[1] + 8, total(build(1, y))) + 0, 14)) + f(y), total(l) + total(l))
