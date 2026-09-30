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
    return 15


def h1(a, b):
    x = b
    return h0(h0(h0(x, 19), h0(10, b)), 15)


def h2(a, b):
    x = total(build(0, total(build(1, (19 if b < a else 11)))))
    return x


def main():
    y = (pair(pair(6, 7)[0], h1(10, 4))[0] if pair(ap(lambda x: x * 7 + 0, 18), ap(lambda x: x * 9 + 5, 11))[1] < h1(7, ap(lambda x: x * 6 + 4, 13)) else pair(2, (4 if 4 < 9 else 4))[1])
    f = lambda x: x * ap(lambda x: x * y + 1, 3) + y
    l = build(0, y)
    return (pair(pair(h1(18, y), (8 if y < y else 9))[1], ap(lambda x: x * 8 + 4, 18))[0], 0, f(pair(pair(h1(18, y), (8 if y < y else 9))[1], ap(lambda x: x * 8 + 4, 18))[0]) + f(y), total(l) + total(l))
