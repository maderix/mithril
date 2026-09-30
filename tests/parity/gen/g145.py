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
    x = ((b & pair(a, 0)[1]) - b)
    return (total(build(4, (3 if 7 < x else 19))) if total(build(1, pair(9, a)[1])) < 17 else 8)


def h1(a, b):
    x = pair(h0(pair(b, 14)[0], h0(16, 3)), h0((a * b), (14 & a)))[0]
    return pair(b, 2)[0]


def h2(a, b):
    x = ((b if (8 if b < a else 18) < h1(b, b) else h0(b, b)) if 18 < pair((a & 11), pair(b, 17)[0])[0] else pair(total(build(6, a)), (19 if b < 19 else 18))[1])
    return (pair(ap(lambda x: x * 7 + 6, 2), ap(lambda x: x * b + 5, 0))[1] if (ap(lambda x: x * b + 8, x) - (b + a)) < x else h1(h0(a, a), a))


def main():
    y = ((5 ^ ap(lambda x: x * 10 + 5, 4)) if (total(build(4, 14)) if (16 * 8) < 5 else (10 if 17 < 4 else 2)) < total(build(1, (9 if 12 < 1 else 15))) else ((12 - 7) if 10 < 7 else total(build(3, 6))))
    f = lambda x: x * total(build(3, h1(16, 13))) + y
    l = build(2, y)
    return (h1(y, pair(4, total(build(1, 4)))[1]), ap(lambda x: x * (13 + (y if 6 < 17 else 18)) + 6, ap(lambda x: x * h0(y, y) + 3, (y & 7))), f(h1(y, pair(4, total(build(1, 4)))[1])) + f(y), total(l) + total(l))
