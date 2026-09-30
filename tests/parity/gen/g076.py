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
    x = a
    return (total(build(1, pair(3, b)[0])) - total(build(0, (b if 12 < b else a))))


def h1(a, b):
    x = ap(lambda x: x * pair(total(build(4, 8)), (a ^ 15))[1] + 3, a)
    return h0(ap(lambda x: x * b + 8, h0(15, 12)), (h0(15, a) ^ pair(19, 10)[1]))


def h2(a, b):
    x = (total(build(0, (a if b < b else b))) if ap(lambda x: x * ap(lambda x: x * a + 0, a) + 4, h1(a, a)) < (total(build(0, 15)) & ap(lambda x: x * 16 + 4, 5)) else b)
    return ap(lambda x: x * total(build(2, ap(lambda x: x * 1 + 5, b))) + 8, 3)


def main():
    y = 11
    f = lambda x: x * y + y
    l = build(5, y)
    return (total(build(4, 17)), (y * ap(lambda x: x * h1(y, 16) + 1, y)), f(total(build(4, 17))) + f(y), total(l) + total(l))
