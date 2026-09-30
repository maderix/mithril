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
    x = ap(lambda x: x * (1 & b) + 8, pair((b if a < 16 else b), pair(b, 10)[1])[1])
    return b


def h1(a, b):
    x = 6
    return 7


def h2(a, b):
    x = (h1(a, 3) ^ total(build(3, b)))
    return pair(total(build(3, b)), ap(lambda x: x * (x if 4 < x else 4) + 4, 11))[0]


def main():
    y = 19
    f = lambda x: x * ((y - 10) if (y & 16) < ap(lambda x: x * 15 + 5, 17) else (13 & y)) + y
    l = build(1, y)
    return (y, y, f(y) + f(y), total(l) + total(l))
