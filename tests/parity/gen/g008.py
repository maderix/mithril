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
    x = 6
    return ap(lambda x: x * (total(build(4, x)) if total(build(0, 9)) < (8 if 16 < b else x) else b) + 7, ap(lambda x: x * a + 7, (0 - b)))


def h1(a, b):
    x = b
    return 11


def h2(a, b):
    x = (ap(lambda x: x * pair(a, 18)[0] + 0, pair(4, 7)[0]) * h1((11 ^ b), pair(a, 1)[0]))
    return (a - total(build(2, a)))


def main():
    y = pair(pair((19 if 14 < 0 else 9), pair(6, 18)[1])[0], h2(pair(19, 4)[1], 4))[1]
    f = lambda x: x * (total(build(6, 12)) if y < ap(lambda x: x * 17 + 7, y) else (y if 4 < y else 15)) + y
    l = build(0, y)
    return (1, ap(lambda x: x * ap(lambda x: x * (y if y < y else y) + 8, pair(18, 1)[0]) + 3, total(build(4, ap(lambda x: x * y + 0, y)))), f(1) + f(y), total(l) + total(l))
