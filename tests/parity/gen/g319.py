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
    x = pair(11, total(build(1, pair(0, 19)[0])))[0]
    return ap(lambda x: x * a + 7, 2)


def h1(a, b):
    x = (pair(pair(6, b)[0], h0(b, a))[0] * pair(ap(lambda x: x * a + 8, 6), (a - a))[1])
    return ((ap(lambda x: x * 6 + 6, 13) ^ (x - 7)) if 19 < ap(lambda x: x * a + 8, ap(lambda x: x * 19 + 3, b)) else total(build(1, total(build(3, 14)))))


def h2(a, b):
    x = (((b * b) & total(build(5, a))) if (pair(5, 1)[1] if pair(a, 9)[1] < a else total(build(0, b))) < pair(pair(17, 7)[1], total(build(6, a)))[1] else pair(h0(8, 18), pair(b, a)[0])[1])
    return (11 + h1((x ^ b), ap(lambda x: x * b + 8, 18)))


def main():
    y = (pair(total(build(5, 5)), 3)[0] * (pair(2, 3)[0] + (7 ^ 5)))
    f = lambda x: x * pair(h2(y, 15), (16 ^ 0))[1] + y
    l = build(3, y)
    return (ap(lambda x: x * 4 + 6, ap(lambda x: x * (18 & 8) + 6, pair(3, 2)[0])), h2(6, h2(h0(4, y), (13 if y < 17 else y))), f(ap(lambda x: x * 4 + 6, ap(lambda x: x * (18 & 8) + 6, pair(3, 2)[0]))) + f(y), total(l) + total(l))
