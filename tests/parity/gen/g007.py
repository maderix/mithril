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
    x = (pair(pair(15, 1)[0], total(build(3, 9)))[1] * pair(ap(lambda x: x * a + 5, a), (19 * 18))[0])
    return b


def h1(a, b):
    x = h0(b, (18 if h0(b, 6) < (b if b < b else b) else 7))
    return ((total(build(3, a)) if (a ^ 15) < ap(lambda x: x * 1 + 0, 4) else (8 if 14 < b else b)) ^ h0((b if 6 < b else b), (x & b)))


def h2(a, b):
    x = 14
    return total(build(2, total(build(0, ap(lambda x: x * 18 + 8, 6)))))


def main():
    y = (ap(lambda x: x * 4 + 5, h0(17, 10)) ^ pair(ap(lambda x: x * 18 + 5, 10), (4 & 13))[1])
    f = lambda x: x * (h2(18, 10) + total(build(6, y))) + y
    l = build(2, y)
    return (h2((pair(5, y)[0] - total(build(4, y))), 18), pair(ap(lambda x: x * (4 + y) + 3, h2(2, y)), ap(lambda x: x * ap(lambda x: x * y + 3, y) + 8, h2(y, 6)))[1], f(h2((pair(5, y)[0] - total(build(4, y))), 18)) + f(y), total(l) + total(l))
