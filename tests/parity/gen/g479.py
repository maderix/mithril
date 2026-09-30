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
    x = ap(lambda x: x * total(build(5, 11)) + 3, (total(build(5, a)) & (b if 19 < a else a)))
    return total(build(3, (19 if b < pair(x, 18)[0] else pair(a, 10)[0])))


def h1(a, b):
    x = 18
    return ap(lambda x: x * (total(build(0, a)) - (a & 9)) + 4, total(build(1, total(build(0, x)))))


def h2(a, b):
    x = 2
    return ap(lambda x: x * a + 1, (h1(x, x) if (7 - x) < x else h0(3, 13)))


def main():
    y = (((10 ^ 7) if (7 & 4) < 14 else (15 - 13)) + (ap(lambda x: x * 1 + 8, 0) if h1(0, 11) < 17 else (15 if 7 < 0 else 13)))
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * y + 5, 17) + 8, (9 - 4)) + y
    l = build(5, y)
    return (7, (total(build(1, ap(lambda x: x * y + 4, y))) if ((19 if 8 < 11 else y) if h0(8, 4) < h1(y, 0) else (y if y < 11 else y)) < pair(total(build(3, y)), pair(9, 6)[1])[0] else ((y if 2 < 2 else y) + pair(y, y)[0])), f(7) + f(y), total(l) + total(l))
