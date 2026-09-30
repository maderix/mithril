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
    x = pair(2, ap(lambda x: x * (19 if 9 < 5 else b) + 0, pair(b, 17)[0]))[0]
    return a


def h1(a, b):
    x = 15
    return 3


def h2(a, b):
    x = (b if total(build(1, (a + 2))) < ((12 + b) & a) else (h0(7, a) ^ h0(1, a)))
    return (h1((b if 12 < 2 else a), pair(17, a)[1]) - a)


def main():
    y = h1(((19 ^ 8) ^ ap(lambda x: x * 7 + 6, 12)), (ap(lambda x: x * 19 + 4, 2) if total(build(5, 14)) < (15 + 14) else 10))
    f = lambda x: x * total(build(1, pair(0, 0)[0])) + y
    l = build(1, y)
    return (total(build(2, total(build(3, (18 - 7))))), total(build(4, total(build(6, (10 & 12))))), f(total(build(2, total(build(3, (18 - 7)))))) + f(y), total(l) + total(l))
