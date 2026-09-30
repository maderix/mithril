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
    x = total(build(2, total(build(4, 10))))
    return ap(lambda x: x * 1 + 7, 15)


def h1(a, b):
    x = (7 if pair((4 if 2 < b else 3), h0(b, a))[0] < a else (pair(5, b)[1] * h0(b, a)))
    return total(build(2, ap(lambda x: x * (13 if a < b else 10) + 0, (3 + 6))))


def h2(a, b):
    x = (pair(pair(a, 14)[0], pair(6, a)[1])[1] * (a - (10 if a < 16 else 17)))
    return (pair(total(build(1, 11)), ap(lambda x: x * a + 3, x))[0] + a)


def main():
    y = (total(build(5, ap(lambda x: x * 4 + 7, 7))) if (1 if 5 < pair(4, 0)[1] else h2(8, 8)) < ap(lambda x: x * (16 if 2 < 1 else 2) + 0, 18) else (18 & h1(2, 3)))
    f = lambda x: x * y + y
    l = build(6, y)
    return (h0(pair((y ^ 2), 4)[0], 1), 9, f(h0(pair((y ^ 2), 4)[0], 1)) + f(y), total(l) + total(l))
