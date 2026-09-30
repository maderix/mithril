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
    x = 19
    return ((ap(lambda x: x * 12 + 2, x) ^ pair(b, b)[1]) if 5 < total(build(1, 8)) else 4)


def h1(a, b):
    x = ((16 if total(build(4, 0)) < (b if 8 < 17 else 7) else (b if a < 13 else b)) ^ total(build(5, (2 ^ a))))
    return ap(lambda x: x * ap(lambda x: x * total(build(0, b)) + 5, b) + 8, 3)


def h2(a, b):
    x = (5 ^ ap(lambda x: x * 10 + 6, h0(17, b)))
    return total(build(4, ap(lambda x: x * b + 5, 10)))


def main():
    y = h1(total(build(4, (1 ^ 9))), 5)
    f = lambda x: x * pair(16, h0(0, y))[0] + y
    l = build(2, y)
    return (10, total(build(0, 2)), f(10) + f(y), total(l) + total(l))
