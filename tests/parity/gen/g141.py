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
    x = total(build(2, pair((6 if 17 < 13 else b), pair(a, 0)[0])[0]))
    return x


def h1(a, b):
    x = ap(lambda x: x * b + 3, b)
    return (((x * 19) if (x - x) < a else h0(b, 0)) if total(build(3, h0(19, 17))) < ap(lambda x: x * ap(lambda x: x * x + 8, 0) + 8, h0(b, 1)) else 1)


def h2(a, b):
    x = total(build(5, total(build(0, (9 ^ 14)))))
    return b


def main():
    y = 1
    f = lambda x: x * ap(lambda x: x * (y if 19 < 9 else y) + 1, ap(lambda x: x * 18 + 5, y)) + y
    l = build(6, y)
    return (y, 17, f(y) + f(y), total(l) + total(l))
