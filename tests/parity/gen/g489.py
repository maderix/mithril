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
    x = ap(lambda x: x * pair((13 if b < b else a), ap(lambda x: x * a + 0, 8))[0] + 4, (6 * (b if a < a else b)))
    return ap(lambda x: x * a + 5, 4)


def h1(a, b):
    x = a
    return a


def h2(a, b):
    x = (a - ap(lambda x: x * (8 if a < 9 else 12) + 3, 13))
    return h1(5, (ap(lambda x: x * a + 0, 12) if ap(lambda x: x * 5 + 1, 16) < ap(lambda x: x * 14 + 6, x) else pair(b, x)[1]))


def main():
    y = ap(lambda x: x * (total(build(3, 12)) + 3) + 6, total(build(5, (11 if 3 < 17 else 9))))
    f = lambda x: x * total(build(5, pair(12, 8)[1])) + y
    l = build(5, y)
    return ((total(build(3, ap(lambda x: x * 10 + 5, 12))) if (total(build(3, y)) & total(build(1, y))) < pair(y, 8)[0] else 11), 17, f((total(build(3, ap(lambda x: x * 10 + 5, 12))) if (total(build(3, y)) & total(build(1, y))) < pair(y, 8)[0] else 11)) + f(y), total(l) + total(l))
