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
    x = total(build(6, total(build(0, b))))
    return x


def h1(a, b):
    x = (ap(lambda x: x * (19 if 16 < 12 else 7) + 5, (4 if 0 < 7 else 16)) - a)
    return total(build(2, pair(total(build(6, x)), (11 if 15 < 3 else 16))[0]))


def h2(a, b):
    x = pair(0, 15)[1]
    return ap(lambda x: x * ap(lambda x: x * h1(a, a) + 4, (b if 11 < 17 else 1)) + 6, (total(build(6, 4)) if ap(lambda x: x * 3 + 4, 8) < (18 - a) else ap(lambda x: x * 9 + 3, x)))


def main():
    y = 3
    f = lambda x: x * total(build(1, pair(y, 14)[0])) + y
    l = build(6, y)
    return ((((y if y < y else y) & (5 - y)) if (11 if (y - 8) < pair(y, y)[0] else (13 if 0 < y else y)) < total(build(5, ap(lambda x: x * y + 8, y))) else h1(total(build(0, 15)), pair(y, y)[0])), 5, f((((y if y < y else y) & (5 - y)) if (11 if (y - 8) < pair(y, y)[0] else (13 if 0 < y else y)) < total(build(5, ap(lambda x: x * y + 8, y))) else h1(total(build(0, 15)), pair(y, y)[0]))) + f(y), total(l) + total(l))
