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
    x = a
    return total(build(3, 15))


def h1(a, b):
    x = pair(pair(ap(lambda x: x * a + 3, 11), b)[1], ap(lambda x: x * 16 + 8, a))[0]
    return h0(total(build(3, total(build(4, b)))), 18)


def h2(a, b):
    x = a
    return h0(h0(ap(lambda x: x * b + 5, 10), (x if a < 3 else 6)), ap(lambda x: x * b + 1, total(build(2, b))))


def main():
    y = h2((pair(4, 1)[0] & (17 & 7)), 6)
    f = lambda x: x * (15 if y < 9 else (18 - y)) + y
    l = build(6, y)
    return (h0(total(build(1, (19 * 11))), total(build(4, 0))), total(build(3, pair(0, pair(16, 14)[0])[0])), f(h0(total(build(1, (19 * 11))), total(build(4, 0)))) + f(y), total(l) + total(l))
