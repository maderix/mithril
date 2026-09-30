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
    x = pair(17, ((b * a) & 3))[0]
    return (ap(lambda x: x * 2 + 0, 5) if ap(lambda x: x * (1 if 19 < x else a) + 0, (13 * x)) < total(build(4, x)) else 5)


def h1(a, b):
    x = b
    return (total(build(5, ap(lambda x: x * 4 + 2, 0))) if x < total(build(2, 0)) else h0(ap(lambda x: x * 11 + 4, b), ap(lambda x: x * 18 + 8, 4)))


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * b + 2, (1 if b < a else 16)) + 2, a)
    return ap(lambda x: x * (1 if total(build(5, 1)) < h0(a, 6) else ap(lambda x: x * 6 + 6, a)) + 3, h0(h1(a, b), b))


def main():
    y = total(build(5, ((19 + 9) ^ 7)))
    f = lambda x: x * y + y
    l = build(2, y)
    return ((total(build(0, 7)) if y < y else (y if (y ^ y) < h0(0, y) else total(build(1, 11)))), ap(lambda x: x * ap(lambda x: x * total(build(6, 0)) + 1, ap(lambda x: x * 10 + 8, 12)) + 0, pair((y ^ 12), total(build(6, 1)))[0]), f((total(build(0, 7)) if y < y else (y if (y ^ y) < h0(0, y) else total(build(1, 11))))) + f(y), total(l) + total(l))
