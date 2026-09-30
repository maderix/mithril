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
    x = total(build(5, (11 - b)))
    return 3


def h1(a, b):
    x = (pair((2 ^ a), ap(lambda x: x * b + 7, 2))[0] if 0 < ap(lambda x: x * 14 + 2, total(build(0, b))) else ap(lambda x: x * 8 + 6, (5 & b)))
    return total(build(4, total(build(2, total(build(0, 11))))))


def h2(a, b):
    x = pair(a, total(build(4, (2 * 13))))[1]
    return pair(h0(a, total(build(6, 1))), x)[1]


def main():
    y = pair(pair(ap(lambda x: x * 0 + 4, 8), h2(0, 0))[1], (ap(lambda x: x * 17 + 3, 12) if ap(lambda x: x * 15 + 5, 17) < 19 else (12 if 0 < 5 else 8)))[1]
    f = lambda x: x * ap(lambda x: x * (10 + y) + 4, y) + y
    l = build(0, y)
    return (((7 if (16 + 13) < ap(lambda x: x * 1 + 1, y) else y) * ap(lambda x: x * h1(15, 18) + 7, 16)), pair((y if y < ap(lambda x: x * 9 + 1, y) else ap(lambda x: x * y + 8, y)), ((y if 8 < 18 else y) if ap(lambda x: x * y + 4, y) < pair(0, y)[1] else (y if 12 < y else 19)))[0], f(((7 if (16 + 13) < ap(lambda x: x * 1 + 1, y) else y) * ap(lambda x: x * h1(15, 18) + 7, 16))) + f(y), total(l) + total(l))
