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
    x = 15
    return total(build(6, ap(lambda x: x * ap(lambda x: x * 8 + 3, b) + 1, 15)))


def h1(a, b):
    x = total(build(6, h0((b ^ b), h0(a, b))))
    return total(build(2, total(build(2, 12))))


def h2(a, b):
    x = total(build(2, b))
    return (total(build(5, ap(lambda x: x * 6 + 4, a))) if (a - ap(lambda x: x * a + 4, b)) < total(build(5, total(build(4, a)))) else h0(total(build(0, b)), h1(b, x)))


def main():
    y = 15
    f = lambda x: x * ap(lambda x: x * 3 + 2, y) + y
    l = build(1, y)
    return (pair((ap(lambda x: x * 2 + 8, y) ^ (y ^ 6)), total(build(4, (16 * 18))))[1], h1(ap(lambda x: x * h0(18, y) + 8, pair(19, 0)[0]), 18), f(pair((ap(lambda x: x * 2 + 8, y) ^ (y ^ 6)), total(build(4, (16 * 18))))[1]) + f(y), total(l) + total(l))
