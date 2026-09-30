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
    x = b
    return 13


def h1(a, b):
    x = ((19 ^ (11 if 5 < b else a)) - a)
    return ap(lambda x: x * total(build(2, b)) + 7, (6 * h0(18, b)))


def h2(a, b):
    x = (pair((a + 9), h1(b, 3))[0] if ((a if b < 8 else 5) if ap(lambda x: x * 8 + 2, 10) < total(build(4, 17)) else (a ^ a)) < pair((17 ^ 5), pair(a, 14)[1])[0] else pair(17, (4 if 4 < 18 else b))[0])
    return ap(lambda x: x * 6 + 4, (total(build(6, x)) if 6 < (b ^ a) else h0(b, b)))


def main():
    y = (ap(lambda x: x * h2(11, 1) + 6, 4) & ap(lambda x: x * (1 if 9 < 0 else 7) + 7, total(build(4, 3))))
    f = lambda x: x * ((y if y < y else 6) - total(build(0, y))) + y
    l = build(3, y)
    return (((ap(lambda x: x * 7 + 8, 17) - pair(y, y)[0]) if pair(2, y)[1] < 6 else 8), y, f(((ap(lambda x: x * 7 + 8, 17) - pair(y, y)[0]) if pair(2, y)[1] < 6 else 8)) + f(y), total(l) + total(l))
