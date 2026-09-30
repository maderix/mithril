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
    return 18


def h1(a, b):
    x = ap(lambda x: x * 11 + 6, ((12 * a) if pair(a, b)[0] < (1 + 1) else total(build(3, 9))))
    return 9


def h2(a, b):
    x = (pair(a, h0(b, 15))[0] * h1(total(build(1, 16)), ap(lambda x: x * a + 3, 13)))
    return 18


def main():
    y = total(build(1, h1(ap(lambda x: x * 16 + 3, 16), pair(3, 4)[0])))
    f = lambda x: x * (12 if pair(18, y)[1] < 1 else (1 ^ 4)) + y
    l = build(5, y)
    return (((h1(17, 7) + h2(y, y)) if (ap(lambda x: x * 1 + 8, y) ^ h1(11, 15)) < ap(lambda x: x * y + 1, total(build(4, 14))) else ap(lambda x: x * (y if 8 < y else y) + 2, (19 ^ y))), pair(((y ^ 14) if (2 if y < y else y) < 1 else 6), pair((y ^ y), total(build(5, 5)))[0])[0], f(((h1(17, 7) + h2(y, y)) if (ap(lambda x: x * 1 + 8, y) ^ h1(11, 15)) < ap(lambda x: x * y + 1, total(build(4, 14))) else ap(lambda x: x * (y if 8 < y else y) + 2, (19 ^ y)))) + f(y), total(l) + total(l))
