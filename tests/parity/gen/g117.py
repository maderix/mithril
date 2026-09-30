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
    x = (9 - pair(total(build(6, b)), 5)[1])
    return 17


def h1(a, b):
    x = total(build(1, total(build(0, pair(b, a)[1]))))
    return 16


def h2(a, b):
    x = (((16 + 10) ^ 17) - a)
    return total(build(2, (pair(5, 1)[1] if b < pair(13, a)[1] else pair(17, x)[1])))


def main():
    y = (1 if (ap(lambda x: x * 1 + 7, 8) if pair(4, 12)[0] < 13 else ap(lambda x: x * 18 + 7, 15)) < (11 if 8 < (8 & 11) else (3 ^ 5)) else pair(3, 8)[1])
    f = lambda x: x * ap(lambda x: x * total(build(3, 14)) + 8, y) + y
    l = build(5, y)
    return (6, h0(ap(lambda x: x * (1 if y < 17 else 9) + 7, 18), total(build(5, 10))), f(6) + f(y), total(l) + total(l))
