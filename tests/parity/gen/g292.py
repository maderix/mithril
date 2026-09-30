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
    x = ((total(build(2, 4)) if (7 if 9 < a else a) < ap(lambda x: x * 19 + 2, 3) else (16 ^ 4)) & b)
    return 0


def h1(a, b):
    x = (total(build(5, ap(lambda x: x * 18 + 7, 14))) ^ h0(total(build(2, 10)), a))
    return (ap(lambda x: x * total(build(1, b)) + 3, ap(lambda x: x * 15 + 1, 5)) if 8 < ap(lambda x: x * h0(a, 18) + 4, (b if a < b else 0)) else 8)


def h2(a, b):
    x = total(build(3, total(build(5, 2))))
    return a


def main():
    y = total(build(1, ap(lambda x: x * ap(lambda x: x * 6 + 5, 13) + 5, pair(18, 4)[1])))
    f = lambda x: x * ((6 + y) & pair(1, 0)[1]) + y
    l = build(2, y)
    return (y, total(build(6, 4)), f(y) + f(y), total(l) + total(l))
