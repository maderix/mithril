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
    x = ((pair(a, a)[0] if a < ap(lambda x: x * 4 + 7, b) else total(build(3, b))) + (total(build(3, a)) if (a * 15) < 16 else ap(lambda x: x * a + 8, 2)))
    return (total(build(1, 14)) & 7)


def h1(a, b):
    x = 16
    return 6


def h2(a, b):
    x = h1(total(build(3, pair(a, 8)[0])), (ap(lambda x: x * b + 1, b) + total(build(5, a))))
    return (a + (pair(2, 3)[1] if ap(lambda x: x * 19 + 1, 6) < h0(b, b) else (15 ^ a)))


def main():
    y = pair((pair(19, 14)[0] if h1(0, 17) < h0(8, 5) else (10 ^ 2)), pair((11 if 4 < 9 else 10), h0(6, 15))[1])[0]
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * y + 7, y) + 2, total(build(3, 6))) + y
    l = build(2, y)
    return (((1 if h2(12, y) < total(build(1, 19)) else total(build(3, 11))) if total(build(0, y)) < y else ap(lambda x: x * total(build(4, y)) + 3, pair(2, y)[1])), 2, f(((1 if h2(12, y) < total(build(1, 19)) else total(build(3, 11))) if total(build(0, y)) < y else ap(lambda x: x * total(build(4, y)) + 3, pair(2, y)[1]))) + f(y), total(l) + total(l))
