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
    x = 11
    return a


def h1(a, b):
    x = h0(total(build(5, h0(13, 8))), pair((a if 16 < b else a), h0(2, b))[0])
    return pair(total(build(3, ap(lambda x: x * 4 + 6, x))), pair((13 if x < b else b), pair(9, b)[0])[0])[0]


def h2(a, b):
    x = total(build(0, h0((a ^ 19), (1 if 5 < b else 18))))
    return 4


def main():
    y = (ap(lambda x: x * (5 if 3 < 0 else 1) + 0, h1(19, 13)) if (ap(lambda x: x * 17 + 5, 14) if pair(18, 10)[0] < total(build(6, 9)) else total(build(0, 0))) < pair((10 * 5), ap(lambda x: x * 7 + 7, 12))[0] else h2(13, 14))
    f = lambda x: x * (pair(16, 8)[1] - h2(12, y)) + y
    l = build(3, y)
    return (ap(lambda x: x * pair(9, total(build(6, 17)))[1] + 5, (pair(10, 3)[1] * 3)), ap(lambda x: x * h2(ap(lambda x: x * y + 4, 19), h2(y, y)) + 5, 9), f(ap(lambda x: x * pair(9, total(build(6, 17)))[1] + 5, (pair(10, 3)[1] * 3))) + f(y), total(l) + total(l))
