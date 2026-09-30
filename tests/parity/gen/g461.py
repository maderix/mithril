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
    x = (total(build(6, (a if b < b else b))) + 1)
    return ap(lambda x: x * 16 + 0, b)


def h1(a, b):
    x = pair(h0(h0(6, 14), a), ((a if a < 13 else b) if (b if b < b else b) < b else 5))[0]
    return (b & ap(lambda x: x * h0(a, b) + 1, pair(19, 12)[0]))


def h2(a, b):
    x = 9
    return 2


def main():
    y = 9
    f = lambda x: x * y + y
    l = build(6, y)
    return (y, 2, f(y) + f(y), total(l) + total(l))
