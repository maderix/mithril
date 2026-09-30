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
    x = ap(lambda x: x * total(build(6, total(build(6, 0)))) + 8, a)
    return 3


def h1(a, b):
    x = ap(lambda x: x * 4 + 1, h0(b, b))
    return 4


def h2(a, b):
    x = 12
    return ((h1(x, 16) if 4 < (1 + 4) else ap(lambda x: x * 19 + 3, 2)) if 19 < h1(total(build(2, b)), total(build(0, 12))) else ((17 * x) if (11 if x < 1 else 15) < (x if b < b else b) else (b if a < 11 else 6)))


def main():
    y = pair(3, (total(build(3, 16)) if (17 if 13 < 0 else 18) < total(build(4, 1)) else (11 + 18)))[0]
    f = lambda x: x * total(build(2, (10 if 5 < 6 else y))) + y
    l = build(2, y)
    return ((ap(lambda x: x * (11 - y) + 4, (1 & y)) ^ y), h2(ap(lambda x: x * (11 * y) + 6, (17 & y)), ((y if 0 < 1 else y) * 7)), f((ap(lambda x: x * (11 - y) + 4, (1 & y)) ^ y)) + f(y), total(l) + total(l))
