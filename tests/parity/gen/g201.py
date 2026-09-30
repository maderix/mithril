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
    x = ap(lambda x: x * total(build(0, (12 & 3))) + 8, ((1 if 19 < 2 else b) if (3 + a) < (15 if b < a else 15) else (9 + 6)))
    return 19


def h1(a, b):
    x = (17 if b < ap(lambda x: x * (b if a < 9 else 0) + 6, 19) else 17)
    return total(build(6, total(build(3, a))))


def h2(a, b):
    x = h0((4 - (10 if a < b else 13)), pair(4, 9)[1])
    return pair((ap(lambda x: x * 19 + 5, x) if total(build(1, 11)) < (x if 0 < x else x) else total(build(2, 16))), h1(x, 3))[1]


def main():
    y = h1(0, 10)
    f = lambda x: x * (pair(15, 0)[0] * (17 - 15)) + y
    l = build(4, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
