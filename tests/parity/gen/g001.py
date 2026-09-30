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
    x = ap(lambda x: x * total(build(0, 8)) + 3, pair((1 * b), total(build(0, a)))[1])
    return pair(a, pair((b & b), ap(lambda x: x * x + 8, 16))[1])[0]


def h1(a, b):
    x = total(build(6, (14 if (15 - 17) < ap(lambda x: x * a + 0, a) else (18 + b))))
    return total(build(3, 3))


def h2(a, b):
    x = ap(lambda x: x * (ap(lambda x: x * 1 + 1, 17) if a < h0(b, 2) else 6) + 2, a)
    return (ap(lambda x: x * h0(b, 2) + 5, h0(b, a)) ^ (pair(x, x)[1] if pair(15, a)[1] < (10 if x < a else a) else b))


def main():
    y = (ap(lambda x: x * h0(10, 3) + 6, pair(18, 1)[1]) - total(build(6, (2 + 13))))
    f = lambda x: x * y + y
    l = build(6, y)
    return (pair(h0((15 if 5 < 3 else y), 14), (ap(lambda x: x * 15 + 1, y) if 5 < 10 else ap(lambda x: x * 14 + 3, y)))[0], (ap(lambda x: x * (y if y < 1 else y) + 3, h0(y, y)) * y), f(pair(h0((15 if 5 < 3 else y), 14), (ap(lambda x: x * 15 + 1, y) if 5 < 10 else ap(lambda x: x * 14 + 3, y)))[0]) + f(y), total(l) + total(l))
