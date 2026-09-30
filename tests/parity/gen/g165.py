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
    x = (ap(lambda x: x * a + 3, 9) - (9 if pair(7, 19)[0] < (12 & b) else b))
    return pair(3, (b - ap(lambda x: x * 19 + 5, a)))[1]


def h1(a, b):
    x = (18 ^ 10)
    return total(build(6, (x if h0(a, a) < (4 if 16 < 5 else x) else (18 & 18))))


def h2(a, b):
    x = total(build(1, ap(lambda x: x * pair(14, a)[1] + 6, b)))
    return (11 * (h1(11, 18) if total(build(1, 12)) < 12 else ap(lambda x: x * b + 3, 17)))


def main():
    y = pair(ap(lambda x: x * (19 * 5) + 0, total(build(0, 4))), 17)[0]
    f = lambda x: x * y + y
    l = build(6, y)
    return (((12 * (4 if 4 < 8 else y)) ^ ap(lambda x: x * ap(lambda x: x * 6 + 2, y) + 5, pair(y, y)[1])), total(build(0, y)), f(((12 * (4 if 4 < 8 else y)) ^ ap(lambda x: x * ap(lambda x: x * 6 + 2, y) + 5, pair(y, y)[1]))) + f(y), total(l) + total(l))
