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
    x = ap(lambda x: x * 19 + 8, pair(4, total(build(6, 4)))[1])
    return (ap(lambda x: x * a + 7, 17) + a)


def h1(a, b):
    x = 10
    return h0(ap(lambda x: x * 5 + 8, (x & 17)), h0((8 & a), ap(lambda x: x * 13 + 5, x)))


def h2(a, b):
    x = a
    return total(build(0, (ap(lambda x: x * 19 + 0, a) ^ 19)))


def main():
    y = 16
    f = lambda x: x * total(build(5, ap(lambda x: x * 19 + 7, y))) + y
    l = build(1, y)
    return (h1(h0((14 if 11 < 11 else y), (y - 16)), pair(total(build(2, y)), (y if y < 2 else 16))[0]), h1(1, (19 if y < h0(14, 3) else ap(lambda x: x * y + 5, 2))), f(h1(h0((14 if 11 < 11 else y), (y - 16)), pair(total(build(2, y)), (y if y < 2 else 16))[0])) + f(y), total(l) + total(l))
