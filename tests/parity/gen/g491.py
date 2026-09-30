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
    x = pair((total(build(3, b)) if (16 if 3 < b else 8) < 11 else ap(lambda x: x * 8 + 5, 4)), 8)[1]
    return 19


def h1(a, b):
    x = total(build(5, ((b + 19) if 12 < pair(17, 11)[1] else (3 ^ 17))))
    return total(build(3, total(build(3, ap(lambda x: x * b + 7, a)))))


def h2(a, b):
    x = ap(lambda x: x * ((6 if b < 7 else a) if ap(lambda x: x * a + 1, 10) < h0(a, 17) else h0(b, b)) + 5, (h1(18, 6) if (2 + 18) < b else 5))
    return a


def main():
    y = 17
    f = lambda x: x * h1(pair(18, y)[1], h0(y, 0)) + y
    l = build(1, y)
    return ((total(build(0, ap(lambda x: x * y + 4, y))) * h1(1, h1(y, y))), ap(lambda x: x * (ap(lambda x: x * y + 2, y) - (7 * 17)) + 0, 9), f((total(build(0, ap(lambda x: x * y + 4, y))) * h1(1, h1(y, y)))) + f(y), total(l) + total(l))
