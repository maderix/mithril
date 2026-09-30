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
    x = a
    return 10


def h1(a, b):
    x = pair(a, total(build(6, total(build(4, 10)))))[1]
    return 7


def h2(a, b):
    x = (pair(total(build(6, 16)), h1(b, 13))[0] if total(build(2, a)) < ((15 & 17) if ap(lambda x: x * b + 1, 6) < b else (b if 5 < 17 else 10)) else a)
    return ap(lambda x: x * 11 + 3, 12)


def main():
    y = 1
    f = lambda x: x * pair(h2(y, 15), pair(17, y)[1])[0] + y
    l = build(5, y)
    return (y, (y if total(build(4, (y & y))) < y else pair(ap(lambda x: x * 16 + 6, 7), total(build(4, 19)))[0]), f(y) + f(y), total(l) + total(l))
