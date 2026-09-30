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
    return (total(build(1, 4)) if ap(lambda x: x * 6 + 2, pair(a, b)[1]) < 18 else (ap(lambda x: x * 10 + 5, 2) + total(build(4, 9))))


def h1(a, b):
    x = 14
    return x


def h2(a, b):
    x = ap(lambda x: x * h0(pair(a, 1)[1], 9) + 4, ap(lambda x: x * b + 3, pair(b, 17)[0]))
    return pair(((12 * 0) if a < 0 else h0(x, 11)), (pair(15, x)[0] if total(build(4, 19)) < 11 else pair(b, x)[0]))[0]


def main():
    y = (3 if ap(lambda x: x * 5 + 6, 0) < 0 else 11)
    f = lambda x: x * ap(lambda x: x * 1 + 4, (y ^ y)) + y
    l = build(0, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
