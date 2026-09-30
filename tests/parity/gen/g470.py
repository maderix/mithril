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
    x = (6 & a)
    return total(build(5, ((b if 18 < b else b) if b < total(build(3, 16)) else pair(17, 6)[0])))


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * b + 2, pair(11, b)[1]) + 2, h0((b - b), total(build(5, 3))))
    return h0(x, 1)


def h2(a, b):
    x = 3
    return a


def main():
    y = (((18 if 7 < 2 else 7) if (10 - 13) < (11 & 8) else (16 & 13)) if ((8 * 7) if ap(lambda x: x * 2 + 7, 15) < 19 else h0(11, 13)) < (total(build(2, 9)) - h1(11, 15)) else ((18 * 9) ^ pair(2, 17)[0]))
    f = lambda x: x * pair(ap(lambda x: x * y + 1, y), ap(lambda x: x * y + 8, 14))[1] + y
    l = build(2, y)
    return (7, total(build(0, (pair(y, y)[1] if ap(lambda x: x * 10 + 3, 0) < pair(y, 11)[1] else ap(lambda x: x * y + 6, y)))), f(7) + f(y), total(l) + total(l))
