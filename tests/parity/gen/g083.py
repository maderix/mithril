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
    x = ((7 if (19 * a) < total(build(5, 7)) else (b if a < 13 else b)) * (a if (b if 12 < b else 1) < pair(3, 15)[1] else (12 - 13)))
    return (((b + a) if pair(b, 8)[0] < 16 else total(build(1, x))) + 0)


def h1(a, b):
    x = total(build(6, ((3 ^ a) if ap(lambda x: x * 6 + 7, a) < a else total(build(2, 18)))))
    return h0(5, b)


def h2(a, b):
    x = h1(h1(total(build(5, a)), (15 * 6)), pair(h0(7, b), total(build(4, 11)))[0])
    return (total(build(0, (14 * a))) if total(build(3, (13 & 16))) < h0((14 if x < 4 else a), h0(a, 6)) else pair(total(build(5, 1)), h1(a, a))[0])


def main():
    y = ap(lambda x: x * 1 + 4, total(build(0, 2)))
    f = lambda x: x * y + y
    l = build(5, y)
    return ((h1(ap(lambda x: x * 5 + 7, 8), 3) & pair(total(build(4, 1)), pair(15, 12)[0])[0]), y, f((h1(ap(lambda x: x * 5 + 7, 8), 3) & pair(total(build(4, 1)), pair(15, 12)[0])[0])) + f(y), total(l) + total(l))
