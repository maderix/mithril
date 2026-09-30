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
    x = 13
    return ((ap(lambda x: x * x + 6, 11) + ap(lambda x: x * 12 + 6, a)) * total(build(0, x)))


def h1(a, b):
    x = h0(ap(lambda x: x * a + 4, h0(18, b)), (ap(lambda x: x * a + 4, a) if (16 + a) < b else total(build(4, 2))))
    return b


def h2(a, b):
    x = ((h0(b, 12) if total(build(4, 2)) < 1 else ap(lambda x: x * 7 + 0, 6)) if b < pair(ap(lambda x: x * b + 5, b), a)[1] else a)
    return (h1(pair(5, b)[0], h0(x, 7)) * (h0(b, a) if (1 * x) < (6 * 4) else 16))


def main():
    y = 1
    f = lambda x: x * 4 + y
    l = build(5, y)
    return ((y if (total(build(3, 18)) if h0(14, 4) < 1 else h1(y, 3)) < pair(total(build(4, y)), total(build(2, 1)))[0] else pair(y, (6 if y < y else y))[1]), h0(((y if 9 < 13 else y) & (y * y)), y), f((y if (total(build(3, 18)) if h0(14, 4) < 1 else h1(y, 3)) < pair(total(build(4, y)), total(build(2, 1)))[0] else pair(y, (6 if y < y else y))[1])) + f(y), total(l) + total(l))
