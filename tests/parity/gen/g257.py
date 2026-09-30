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
    x = pair(ap(lambda x: x * (a if 0 < a else 4) + 6, 8), ap(lambda x: x * (6 if 4 < b else b) + 1, total(build(3, b))))[0]
    return ap(lambda x: x * a + 7, ap(lambda x: x * pair(17, x)[1] + 7, (b if 15 < a else b)))


def h1(a, b):
    x = (16 if h0((b * a), pair(b, 11)[0]) < total(build(4, (7 if b < a else a))) else b)
    return (ap(lambda x: x * h0(5, b) + 5, (x & b)) if pair(ap(lambda x: x * 18 + 3, 5), ap(lambda x: x * x + 6, 5))[0] < ap(lambda x: x * b + 7, (x if 3 < x else x)) else total(build(1, pair(5, x)[1])))


def h2(a, b):
    x = 11
    return a


def main():
    y = (ap(lambda x: x * 12 + 2, (6 if 11 < 10 else 9)) & total(build(5, (9 ^ 10))))
    f = lambda x: x * total(build(5, pair(15, 13)[1])) + y
    l = build(4, y)
    return (9, pair(pair(ap(lambda x: x * y + 5, y), ap(lambda x: x * y + 1, y))[1], total(build(1, pair(10, 9)[1])))[0], f(9) + f(y), total(l) + total(l))
