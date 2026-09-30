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
    x = total(build(1, 5))
    return ap(lambda x: x * ((x - a) if 19 < ap(lambda x: x * a + 7, 19) else x) + 8, 7)


def h1(a, b):
    x = ((a & a) if total(build(5, pair(b, 19)[0])) < h0(h0(a, b), (14 * 10)) else total(build(5, h0(a, a))))
    return ap(lambda x: x * (b if a < a else h0(b, a)) + 3, (ap(lambda x: x * 2 + 1, x) ^ (b if 9 < 3 else x)))


def h2(a, b):
    x = total(build(2, h1((14 - 5), ap(lambda x: x * a + 0, 16))))
    return (ap(lambda x: x * (b ^ 19) + 1, 15) + ap(lambda x: x * ap(lambda x: x * x + 0, x) + 6, (x - 9)))


def main():
    y = 10
    f = lambda x: x * ap(lambda x: x * total(build(0, y)) + 0, 13) + y
    l = build(4, y)
    return ((h0(y, (1 if 2 < 15 else 17)) * h1(total(build(1, y)), 7)), 6, f((h0(y, (1 if 2 < 15 else 17)) * h1(total(build(1, y)), 7))) + f(y), total(l) + total(l))
