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
    x = 7
    return 9


def h1(a, b):
    x = 4
    return h0(h0(pair(19, 14)[0], b), ((10 if a < b else 10) if h0(0, a) < (12 + b) else x))


def h2(a, b):
    x = h1(11, 17)
    return h0(h1(total(build(3, 0)), x), x)


def main():
    y = h0((total(build(6, 1)) if (4 ^ 16) < 19 else 11), h2(h0(8, 9), total(build(5, 3))))
    f = lambda x: x * 15 + y
    l = build(0, y)
    return (total(build(2, pair(h0(y, 7), h2(12, y))[1])), ((7 if h1(7, 12) < (y + 2) else y) - y), f(total(build(2, pair(h0(y, 7), h2(12, y))[1]))) + f(y), total(l) + total(l))
