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
    x = ((b if 15 < (b if 2 < a else a) else pair(a, 8)[0]) * ap(lambda x: x * ap(lambda x: x * a + 0, 7) + 1, total(build(5, 13))))
    return ap(lambda x: x * ap(lambda x: x * b + 3, 1) + 6, 2)


def h1(a, b):
    x = (((a & 12) if (17 ^ a) < 0 else h0(7, 3)) + 5)
    return (pair(total(build(4, 10)), total(build(6, 2)))[0] + total(build(0, (a ^ 1))))


def h2(a, b):
    x = h0((pair(14, b)[1] if ap(lambda x: x * 8 + 7, a) < total(build(5, a)) else ap(lambda x: x * 10 + 0, a)), total(build(3, 12)))
    return a


def main():
    y = 16
    f = lambda x: x * ((17 if 12 < 2 else 9) ^ total(build(2, y))) + y
    l = build(0, y)
    return (h2((ap(lambda x: x * 9 + 7, y) if 1 < h2(y, y) else (y if 15 < 1 else y)), 2), y, f(h2((ap(lambda x: x * 9 + 7, y) if 1 < h2(y, y) else (y if 15 < 1 else y)), 2)) + f(y), total(l) + total(l))
