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
    x = (3 * (16 & pair(5, a)[1]))
    return ap(lambda x: x * ap(lambda x: x * b + 5, (18 if a < 3 else b)) + 1, (ap(lambda x: x * a + 8, b) if pair(b, a)[1] < (a ^ a) else pair(0, 10)[0]))


def h1(a, b):
    x = h0(total(build(3, pair(13, 0)[0])), pair(a, ap(lambda x: x * b + 6, a))[1])
    return a


def h2(a, b):
    x = (ap(lambda x: x * 9 + 1, pair(b, 10)[0]) & pair(total(build(4, a)), total(build(4, 14)))[1])
    return 14


def main():
    y = 9
    f = lambda x: x * y + y
    l = build(1, y)
    return (ap(lambda x: x * ((y * y) if total(build(2, y)) < (y if y < y else 18) else (17 * 13)) + 3, y), (y & (pair(15, y)[0] if y < ap(lambda x: x * 7 + 5, 7) else 0)), f(ap(lambda x: x * ((y * y) if total(build(2, y)) < (y if y < y else 18) else (17 * 13)) + 3, y)) + f(y), total(l) + total(l))
