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
    x = pair(pair(b, ap(lambda x: x * a + 3, a))[0], total(build(0, ap(lambda x: x * b + 5, 13))))[1]
    return 12


def h1(a, b):
    x = pair((2 if (a & 8) < h0(b, a) else (19 + a)), total(build(6, pair(5, 9)[0])))[0]
    return ap(lambda x: x * ap(lambda x: x * (6 * b) + 5, (6 if 14 < x else 13)) + 7, a)


def h2(a, b):
    x = total(build(4, b))
    return pair(pair(x, (x & 0))[1], (total(build(0, 19)) + 16))[1]


def main():
    y = pair(h1(5, ap(lambda x: x * 1 + 5, 10)), (3 & pair(3, 7)[1]))[1]
    f = lambda x: x * h0(total(build(0, 6)), 16) + y
    l = build(2, y)
    return (11, (y * pair(ap(lambda x: x * y + 6, 5), (12 - y))[1]), f(11) + f(y), total(l) + total(l))
