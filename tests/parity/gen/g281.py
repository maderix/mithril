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
    x = ((ap(lambda x: x * 3 + 6, a) if 3 < pair(13, b)[0] else a) if total(build(0, 12)) < 6 else pair(b, (a if b < 11 else 6))[0])
    return pair(18, total(build(6, 5)))[1]


def h1(a, b):
    x = (h0(h0(b, b), total(build(2, a))) - 10)
    return (8 + 17)


def h2(a, b):
    x = pair(total(build(5, h1(4, b))), total(build(3, total(build(6, 15)))))[0]
    return (total(build(0, (16 * 6))) - pair(a, (2 if 2 < 10 else b))[0])


def main():
    y = 1
    f = lambda x: x * (total(build(5, y)) if 17 < (19 & 17) else total(build(2, 18))) + y
    l = build(1, y)
    return (15, ap(lambda x: x * ap(lambda x: x * (y + 16) + 6, ap(lambda x: x * 8 + 5, y)) + 6, ((19 if y < 13 else 17) + ap(lambda x: x * y + 5, y))), f(15) + f(y), total(l) + total(l))
