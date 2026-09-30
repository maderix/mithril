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
    x = pair(ap(lambda x: x * 14 + 2, a), a)[1]
    return ((4 ^ 6) & total(build(4, ap(lambda x: x * b + 0, x))))


def h1(a, b):
    x = (total(build(3, total(build(1, b)))) - pair((a ^ 10), total(build(3, 8)))[1])
    return (ap(lambda x: x * 15 + 2, ap(lambda x: x * 12 + 1, 14)) if x < pair((9 ^ b), (x if x < x else 5))[0] else h0(a, 15))


def h2(a, b):
    x = (pair((a if 6 < a else 16), 17)[1] if total(build(3, (b if 3 < a else 15))) < pair(total(build(1, a)), total(build(2, b)))[0] else (h1(a, 8) if 4 < (0 if b < a else 8) else 12))
    return pair(x, 8)[1]


def main():
    y = ap(lambda x: x * h0((12 if 11 < 8 else 18), 6) + 6, 1)
    f = lambda x: x * y + y
    l = build(1, y)
    return (ap(lambda x: x * (8 + total(build(1, 14))) + 1, ((y ^ y) if y < (y if y < y else 11) else total(build(4, 5)))), pair(y, total(build(3, pair(14, y)[0])))[1], f(ap(lambda x: x * (8 + total(build(1, 14))) + 1, ((y ^ y) if y < (y if y < y else 11) else total(build(4, 5))))) + f(y), total(l) + total(l))
