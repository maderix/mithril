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
    x = total(build(6, 13))
    return 19


def h1(a, b):
    x = (h0((18 - 11), h0(15, b)) * (ap(lambda x: x * b + 6, a) if total(build(5, 7)) < (4 if 17 < 2 else 8) else ap(lambda x: x * b + 0, 6)))
    return 4


def h2(a, b):
    x = ap(lambda x: x * 12 + 7, (ap(lambda x: x * a + 5, b) if h1(a, 2) < total(build(4, a)) else total(build(4, 17))))
    return x


def main():
    y = (h0(11, h2(4, 1)) if 10 < 1 else 6)
    f = lambda x: x * ((y - 2) if y < total(build(3, 1)) else ap(lambda x: x * y + 8, 19)) + y
    l = build(6, y)
    return ((total(build(3, y)) if ap(lambda x: x * (y if 14 < y else y) + 8, 19) < y else ap(lambda x: x * (y - y) + 1, ap(lambda x: x * y + 6, y))), 7, f((total(build(3, y)) if ap(lambda x: x * (y if 14 < y else y) + 8, 19) < y else ap(lambda x: x * (y - y) + 1, ap(lambda x: x * y + 6, y)))) + f(y), total(l) + total(l))
