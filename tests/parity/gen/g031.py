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
    x = total(build(2, ((5 if 2 < b else 5) ^ (2 + 1))))
    return ((ap(lambda x: x * a + 1, 9) if total(build(6, 12)) < total(build(4, 13)) else 17) * (pair(10, 0)[0] if (b ^ 17) < ap(lambda x: x * 12 + 0, 6) else 10))


def h1(a, b):
    x = pair(total(build(0, pair(b, 17)[1])), ap(lambda x: x * h0(0, a) + 8, h0(b, 8)))[1]
    return 9


def h2(a, b):
    x = (12 - h1(17, (a & a)))
    return (ap(lambda x: x * pair(x, 12)[0] + 4, (16 if x < 3 else 6)) if pair(12, x)[1] < pair(h1(11, a), total(build(2, x)))[1] else pair(x, ap(lambda x: x * b + 7, x))[0])


def main():
    y = pair(9, 18)[0]
    f = lambda x: x * 17 + y
    l = build(3, y)
    return (h2(y, h2(h2(15, y), ap(lambda x: x * y + 4, y))), pair((total(build(3, y)) + (y + 7)), (y & (7 * y)))[1], f(h2(y, h2(h2(15, y), ap(lambda x: x * y + 4, y)))) + f(y), total(l) + total(l))
