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
    x = total(build(6, 18))
    return 2


def h1(a, b):
    x = h0(1, total(build(4, ap(lambda x: x * a + 2, b))))
    return pair(pair(ap(lambda x: x * b + 7, 3), (x if 7 < x else 0))[0], 13)[1]


def h2(a, b):
    x = (ap(lambda x: x * (11 if 16 < b else a) + 7, h0(8, 14)) if (h1(a, 11) + h0(5, b)) < pair((7 if 17 < b else 19), 16)[0] else h1(ap(lambda x: x * b + 1, b), ap(lambda x: x * a + 7, b)))
    return total(build(3, x))


def main():
    y = (((8 ^ 13) ^ 19) if 8 < pair(13, h1(17, 6))[0] else 5)
    f = lambda x: x * y + y
    l = build(1, y)
    return (total(build(3, (ap(lambda x: x * y + 2, y) if (19 ^ y) < pair(y, 19)[1] else (y if 18 < y else y)))), y, f(total(build(3, (ap(lambda x: x * y + 2, y) if (19 ^ y) < pair(y, 19)[1] else (y if 18 < y else y))))) + f(y), total(l) + total(l))
