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
    x = pair(pair((b + b), a)[1], total(build(4, b)))[1]
    return ap(lambda x: x * (a & pair(6, a)[1]) + 5, ap(lambda x: x * x + 3, 8))


def h1(a, b):
    x = (total(build(2, ap(lambda x: x * b + 3, b))) if pair((b * a), 8)[0] < (ap(lambda x: x * a + 6, 14) if h0(b, 13) < total(build(3, 12)) else 12) else 3)
    return a


def h2(a, b):
    x = (b if 5 < ((1 if b < 13 else a) ^ 13) else (ap(lambda x: x * a + 2, b) if ap(lambda x: x * 11 + 8, 9) < b else ap(lambda x: x * a + 6, 5)))
    return (pair(total(build(6, 0)), 1)[0] + pair((19 if 2 < 16 else 14), pair(3, 3)[0])[0])


def main():
    y = h1(h1(ap(lambda x: x * 15 + 8, 6), 13), ap(lambda x: x * total(build(2, 8)) + 3, (5 & 4)))
    f = lambda x: x * ap(lambda x: x * y + 8, 17) + y
    l = build(1, y)
    return (total(build(1, pair((18 if y < 13 else 14), (19 if 1 < y else y))[1])), total(build(1, 14)), f(total(build(1, pair((18 if y < 13 else 14), (19 if 1 < y else y))[1]))) + f(y), total(l) + total(l))
