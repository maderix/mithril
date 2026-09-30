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
    x = total(build(4, total(build(1, 9))))
    return (4 + total(build(0, 8)))


def h1(a, b):
    x = a
    return pair(14, h0((a * b), (x if 1 < x else 12)))[0]


def h2(a, b):
    x = b
    return 16


def main():
    y = ((16 & pair(19, 3)[1]) if 10 < (9 ^ h2(2, 9)) else ap(lambda x: x * (12 * 3) + 2, (18 - 8)))
    f = lambda x: x * h0(h1(11, y), (11 ^ 0)) + y
    l = build(4, y)
    return (14, y, f(14) + f(y), total(l) + total(l))
