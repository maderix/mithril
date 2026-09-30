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
    return x


def h1(a, b):
    x = (3 if 0 < pair(ap(lambda x: x * b + 1, 0), 19)[1] else 4)
    return (ap(lambda x: x * total(build(2, 6)) + 8, h0(x, a)) if 0 < x else ap(lambda x: x * 4 + 1, ap(lambda x: x * b + 3, 10)))


def h2(a, b):
    x = ap(lambda x: x * b + 6, total(build(6, a)))
    return 17


def main():
    y = (2 if h1(total(build(4, 17)), (6 ^ 14)) < ap(lambda x: x * 2 + 4, h1(0, 12)) else (total(build(6, 18)) - total(build(4, 14))))
    f = lambda x: x * y + y
    l = build(4, y)
    return (16, 3, f(16) + f(y), total(l) + total(l))
