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
    x = 9
    return pair(total(build(1, ap(lambda x: x * x + 7, b))), ap(lambda x: x * pair(b, x)[0] + 5, pair(9, 8)[0]))[0]


def h1(a, b):
    x = b
    return (total(build(1, a)) if (pair(x, x)[0] + ap(lambda x: x * 13 + 0, a)) < pair(ap(lambda x: x * b + 8, 2), h0(a, x))[1] else pair((x if a < 6 else a), (x if 6 < 10 else a))[0])


def h2(a, b):
    x = h0(a, total(build(1, a)))
    return (total(build(5, pair(19, 19)[0])) if pair(a, h0(18, x))[0] < ap(lambda x: x * ap(lambda x: x * 13 + 3, 7) + 1, h1(15, 13)) else 10)


def main():
    y = total(build(1, 11))
    f = lambda x: x * total(build(6, (y ^ 7))) + y
    l = build(4, y)
    return (ap(lambda x: x * (4 if total(build(0, 0)) < (19 if 12 < y else y) else h0(4, y)) + 2, h0(y, (y & y))), y, f(ap(lambda x: x * (4 if total(build(0, 0)) < (19 if 12 < y else y) else h0(4, y)) + 2, h0(y, (y & y)))) + f(y), total(l) + total(l))
