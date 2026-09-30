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
    return x


def h1(a, b):
    x = a
    return total(build(2, b))


def h2(a, b):
    x = pair(total(build(6, (6 + 11))), b)[1]
    return (18 * h0(ap(lambda x: x * b + 4, 14), b))


def main():
    y = h1(pair(15, (8 - 0))[1], (ap(lambda x: x * 8 + 6, 6) if pair(1, 15)[1] < pair(10, 0)[0] else total(build(6, 13))))
    f = lambda x: x * ap(lambda x: x * h1(5, 8) + 4, (y * y)) + y
    l = build(0, y)
    return ((2 * y), 6, f((2 * y)) + f(y), total(l) + total(l))
