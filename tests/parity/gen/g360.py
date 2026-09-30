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
    x = (total(build(0, total(build(6, a)))) & 3)
    return pair(13, 11)[1]


def h1(a, b):
    x = b
    return 11


def h2(a, b):
    x = total(build(4, ap(lambda x: x * (1 if b < 4 else 17) + 4, 0)))
    return (x - (h0(7, x) - 12))


def main():
    y = 6
    f = lambda x: x * total(build(5, total(build(2, 11)))) + y
    l = build(4, y)
    return (pair(ap(lambda x: x * 12 + 1, ap(lambda x: x * y + 4, 19)), h1((8 if y < 15 else 4), h0(15, 15)))[0], y, f(pair(ap(lambda x: x * 12 + 1, ap(lambda x: x * y + 4, 19)), h1((8 if y < 15 else 4), h0(15, 15)))[0]) + f(y), total(l) + total(l))
