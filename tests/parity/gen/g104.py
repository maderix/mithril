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
    x = 4
    return (pair(pair(b, 16)[1], total(build(1, a)))[0] if 6 < pair(a, total(build(1, a)))[0] else 10)


def h1(a, b):
    x = a
    return pair(x, (total(build(2, 13)) if ap(lambda x: x * 10 + 4, x) < h0(x, x) else (x * x)))[0]


def h2(a, b):
    x = (pair((16 + 9), h1(b, 12))[0] if a < (total(build(3, 16)) if (1 ^ 13) < ap(lambda x: x * 2 + 1, a) else (17 + 0)) else h1(ap(lambda x: x * b + 8, a), ap(lambda x: x * 7 + 7, a)))
    return 2


def main():
    y = 14
    f = lambda x: x * total(build(6, 18)) + y
    l = build(1, y)
    return (total(build(5, (pair(y, 12)[0] if y < y else y))), 14, f(total(build(5, (pair(y, 12)[0] if y < y else y)))) + f(y), total(l) + total(l))
