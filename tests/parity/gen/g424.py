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
    x = 3
    return 19


def h1(a, b):
    x = ap(lambda x: x * (b if (b if a < 11 else a) < b else a) + 3, pair((b - a), b)[1])
    return 17


def h2(a, b):
    x = ap(lambda x: x * pair(ap(lambda x: x * 2 + 5, a), ap(lambda x: x * 19 + 4, 15))[1] + 6, ((5 - a) & total(build(3, b))))
    return ap(lambda x: x * (ap(lambda x: x * a + 7, a) if a < 19 else pair(12, x)[1]) + 7, ((a ^ b) if h1(19, x) < (x + 1) else a))


def main():
    y = ((h1(16, 11) - ap(lambda x: x * 8 + 6, 1)) if 3 < h0(5, 1) else h2((3 & 11), (19 * 18)))
    f = lambda x: x * ap(lambda x: x * (12 * 14) + 4, (11 if y < y else y)) + y
    l = build(1, y)
    return (ap(lambda x: x * 0 + 4, total(build(4, pair(2, 19)[1]))), 13, f(ap(lambda x: x * 0 + 4, total(build(4, pair(2, 19)[1])))) + f(y), total(l) + total(l))
