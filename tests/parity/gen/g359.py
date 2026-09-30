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
    return ((9 * (b if 16 < 1 else x)) & 16)


def h1(a, b):
    x = total(build(4, total(build(5, h0(b, 8)))))
    return total(build(2, h0(pair(6, a)[1], 0)))


def h2(a, b):
    x = 5
    return h1(14, x)


def main():
    y = 2
    f = lambda x: x * (pair(17, 17)[1] * y) + y
    l = build(0, y)
    return (pair(pair(pair(1, y)[1], (y * 0))[1], (h2(16, 2) - h2(y, 1)))[0], (h2((y if 12 < 4 else 13), ap(lambda x: x * 18 + 0, y)) if h2(h0(y, 13), 17) < ((y & 2) * (y if y < y else y)) else total(build(6, (8 if 10 < 16 else y)))), f(pair(pair(pair(1, y)[1], (y * 0))[1], (h2(16, 2) - h2(y, 1)))[0]) + f(y), total(l) + total(l))
