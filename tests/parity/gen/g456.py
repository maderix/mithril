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
    x = (a if ((a if 9 < b else b) * total(build(6, b))) < 7 else 6)
    return ap(lambda x: x * 12 + 3, (pair(b, a)[1] ^ (a if 17 < 12 else 19)))


def h1(a, b):
    x = pair(h0(total(build(4, a)), b), (a if total(build(2, 17)) < h0(19, b) else b))[1]
    return (total(build(3, pair(16, 9)[0])) & ((4 if x < x else 14) if total(build(0, b)) < (7 - 19) else 0))


def h2(a, b):
    x = a
    return h0(h0(h1(19, a), total(build(3, 8))), (total(build(0, 10)) if pair(a, a)[1] < pair(b, x)[1] else x))


def main():
    y = 17
    f = lambda x: x * y + y
    l = build(1, y)
    return (y, (y ^ ap(lambda x: x * h2(1, 1) + 1, ap(lambda x: x * y + 1, 12))), f(y) + f(y), total(l) + total(l))
