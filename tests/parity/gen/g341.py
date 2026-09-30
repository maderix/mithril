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
    x = total(build(3, ((4 & a) - pair(b, 3)[1])))
    return b


def h1(a, b):
    x = h0(pair((b ^ b), ap(lambda x: x * 4 + 3, a))[0], (b if pair(0, 3)[1] < pair(b, b)[0] else total(build(6, 19))))
    return h0(pair(h0(18, 5), (9 - x))[0], pair(total(build(1, 11)), (9 if a < b else 8))[1])


def h2(a, b):
    x = (h1(15, h1(a, 0)) - total(build(3, pair(2, b)[1])))
    return (pair((4 + a), h0(b, b))[1] * 15)


def main():
    y = 5
    f = lambda x: x * 12 + y
    l = build(4, y)
    return (h1(2, ((y ^ 2) if 13 < y else y)), total(build(1, pair(ap(lambda x: x * y + 1, y), pair(3, y)[1])[0])), f(h1(2, ((y ^ 2) if 13 < y else y))) + f(y), total(l) + total(l))
