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
    x = 19
    return pair(ap(lambda x: x * (x & x) + 6, (b if b < 17 else a)), ap(lambda x: x * (3 if x < x else x) + 8, (14 if 16 < 16 else x)))[0]


def h1(a, b):
    x = pair(pair(a, h0(2, 11))[1], total(build(3, ap(lambda x: x * a + 2, a))))[1]
    return 13


def h2(a, b):
    x = 12
    return ap(lambda x: x * x + 5, ap(lambda x: x * (b ^ x) + 2, (10 if 19 < 5 else 0)))


def main():
    y = total(build(3, 12))
    f = lambda x: x * ap(lambda x: x * (y if y < y else y) + 0, y) + y
    l = build(5, y)
    return (15, (ap(lambda x: x * ap(lambda x: x * 9 + 4, 9) + 0, (y - 1)) * 1), f(15) + f(y), total(l) + total(l))
