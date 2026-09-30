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
    x = (total(build(1, 2)) * (ap(lambda x: x * 14 + 6, 19) if (b & b) < (b if 9 < a else a) else 14))
    return 17


def h1(a, b):
    x = 5
    return (pair(h0(a, x), (x if 7 < x else b))[1] if 1 < b else h0((12 if x < 8 else 15), (x & 9)))


def h2(a, b):
    x = 17
    return ap(lambda x: x * (h0(x, 10) if ap(lambda x: x * 12 + 0, a) < ap(lambda x: x * 16 + 1, a) else total(build(4, 2))) + 2, (5 if 4 < total(build(3, 2)) else 5))


def main():
    y = pair(pair(13, 4)[1], ((7 if 15 < 17 else 3) ^ pair(6, 7)[1]))[1]
    f = lambda x: x * y + y
    l = build(2, y)
    return ((h2((10 if y < y else y), (y ^ y)) + ap(lambda x: x * y + 4, total(build(0, 18)))), y, f((h2((10 if y < y else y), (y ^ y)) + ap(lambda x: x * y + 4, total(build(0, 18))))) + f(y), total(l) + total(l))
