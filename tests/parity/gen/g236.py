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
    x = ((b * b) if 9 < 14 else 8)
    return total(build(2, (total(build(5, a)) if 6 < 9 else (b - 3))))


def h1(a, b):
    x = ap(lambda x: x * 15 + 2, total(build(2, (14 & 19))))
    return (ap(lambda x: x * h0(14, 17) + 6, (a if b < x else 0)) + total(build(5, 16)))


def h2(a, b):
    x = ap(lambda x: x * (15 if h1(b, 18) < pair(a, 2)[1] else (b if b < b else a)) + 6, (total(build(3, a)) * (a + 8)))
    return ap(lambda x: x * pair(4, 3)[1] + 3, (x ^ ap(lambda x: x * a + 8, 2)))


def main():
    y = ap(lambda x: x * ap(lambda x: x * pair(9, 8)[0] + 1, 8) + 2, ap(lambda x: x * 6 + 3, h2(10, 1)))
    f = lambda x: x * h0((12 * 15), pair(13, y)[0]) + y
    l = build(4, y)
    return (((total(build(4, y)) ^ total(build(4, 10))) if (12 ^ h2(y, y)) < (total(build(2, y)) * h0(13, 13)) else h0((y - 12), (y ^ y))), total(build(0, y)), f(((total(build(4, y)) ^ total(build(4, 10))) if (12 ^ h2(y, y)) < (total(build(2, y)) * h0(13, 13)) else h0((y - 12), (y ^ y)))) + f(y), total(l) + total(l))
