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
    x = a
    return pair(5, (19 if (2 if a < a else x) < 16 else (a + a)))[1]


def h1(a, b):
    x = pair(h0(h0(0, 6), pair(a, a)[1]), total(build(6, b)))[1]
    return (13 if ap(lambda x: x * h0(a, x) + 7, ap(lambda x: x * 4 + 1, 6)) < pair(total(build(1, x)), (x ^ b))[0] else ap(lambda x: x * 1 + 2, 18))


def h2(a, b):
    x = (ap(lambda x: x * pair(2, 0)[1] + 1, a) * (ap(lambda x: x * a + 1, 2) if h1(a, 9) < pair(b, 1)[0] else h1(15, 18)))
    return pair(x, (total(build(5, 8)) if a < pair(a, 18)[1] else (b if b < b else 6)))[0]


def main():
    y = ap(lambda x: x * h1(h1(13, 19), 12) + 4, ap(lambda x: x * (6 ^ 7) + 4, total(build(2, 14))))
    f = lambda x: x * total(build(6, (y ^ y))) + y
    l = build(1, y)
    return (14, (total(build(6, (y if 1 < y else y))) + h1(total(build(3, y)), (11 - y))), f(14) + f(y), total(l) + total(l))
