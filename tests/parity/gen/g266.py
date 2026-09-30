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
    x = 1
    return 14


def h1(a, b):
    x = ap(lambda x: x * total(build(2, 4)) + 0, ap(lambda x: x * (a if a < b else 1) + 0, (b + 6)))
    return ap(lambda x: x * ((1 if b < x else 4) if 11 < 17 else (x if 7 < 12 else a)) + 0, ((0 + 4) & ap(lambda x: x * b + 6, b)))


def h2(a, b):
    x = 12
    return a


def main():
    y = 1
    f = lambda x: x * h0(h0(y, y), y) + y
    l = build(0, y)
    return (((pair(14, y)[1] + (y ^ 19)) ^ y), y, f(((pair(14, y)[1] + (y ^ 19)) ^ y)) + f(y), total(l) + total(l))
