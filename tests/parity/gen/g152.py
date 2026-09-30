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
    x = b
    return total(build(1, (ap(lambda x: x * 7 + 2, b) + (10 * b))))


def h1(a, b):
    x = (pair(h0(10, 17), total(build(2, a)))[1] + ap(lambda x: x * (10 if b < 15 else 5) + 7, (2 & a)))
    return (4 if (total(build(0, 15)) if (b ^ 6) < (a if x < 6 else b) else pair(18, 2)[0]) < pair(b, total(build(2, x)))[1] else total(build(4, pair(b, 12)[0])))


def h2(a, b):
    x = h0(ap(lambda x: x * 16 + 1, h0(b, a)), b)
    return (b - x)


def main():
    y = h2(total(build(1, ap(lambda x: x * 6 + 1, 14))), (13 - (17 if 11 < 18 else 14)))
    f = lambda x: x * 5 + y
    l = build(4, y)
    return (total(build(4, pair(y, 16)[1])), pair(h1((19 * y), y), 9)[0], f(total(build(4, pair(y, 16)[1]))) + f(y), total(l) + total(l))
