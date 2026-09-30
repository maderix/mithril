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
    x = 16
    return (ap(lambda x: x * a + 1, (a + a)) * ((10 - 3) + total(build(0, 18))))


def h1(a, b):
    x = total(build(6, 12))
    return h0(h0(total(build(0, b)), total(build(1, 6))), 1)


def h2(a, b):
    x = 9
    return 11


def main():
    y = (((13 if 18 < 7 else 18) ^ 3) & (h0(15, 0) - pair(12, 16)[0]))
    f = lambda x: x * 16 + y
    l = build(0, y)
    return (ap(lambda x: x * (0 & (9 * 16)) + 7, ap(lambda x: x * (y & y) + 0, ap(lambda x: x * 17 + 1, y))), ap(lambda x: x * (y if ap(lambda x: x * 8 + 7, 12) < (y & 4) else total(build(6, y))) + 3, (ap(lambda x: x * 12 + 8, 17) & y)), f(ap(lambda x: x * (0 & (9 * 16)) + 7, ap(lambda x: x * (y & y) + 0, ap(lambda x: x * 17 + 1, y)))) + f(y), total(l) + total(l))
