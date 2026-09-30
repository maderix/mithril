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
    x = (total(build(2, (b if b < b else b))) - (1 if (b & 13) < pair(12, a)[0] else (11 & 1)))
    return 15


def h1(a, b):
    x = 18
    return (17 & (h0(x, x) & (x & b)))


def h2(a, b):
    x = ap(lambda x: x * pair(h1(19, b), b)[0] + 5, pair(2, h0(16, 5))[0])
    return (16 * (x if (b ^ 19) < (9 ^ 6) else 2))


def main():
    y = (ap(lambda x: x * 13 + 3, h1(17, 19)) if 14 < total(build(6, (11 if 10 < 18 else 12))) else (pair(5, 19)[0] if pair(9, 1)[1] < pair(11, 17)[1] else (4 if 10 < 16 else 7)))
    f = lambda x: x * ap(lambda x: x * (8 - y) + 7, total(build(1, 0))) + y
    l = build(6, y)
    return (pair((total(build(0, y)) + y), 9)[1], y, f(pair((total(build(0, y)) + y), 9)[1]) + f(y), total(l) + total(l))
