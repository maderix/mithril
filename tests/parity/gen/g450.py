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
    x = (ap(lambda x: x * (b ^ 16) + 0, 12) * 19)
    return x


def h1(a, b):
    x = pair(ap(lambda x: x * 5 + 6, h0(b, b)), (b * pair(2, a)[1]))[1]
    return 19


def h2(a, b):
    x = 1
    return ((ap(lambda x: x * 2 + 6, x) if 16 < (1 if b < 13 else 8) else total(build(6, 2))) * (pair(a, 13)[1] & total(build(3, 8))))


def main():
    y = 7
    f = lambda x: x * ((y + y) if (18 & y) < pair(y, 15)[1] else (5 if y < 17 else 11)) + y
    l = build(2, y)
    return (total(build(4, total(build(3, pair(13, y)[1])))), total(build(4, h2(2, (y if 6 < y else 5)))), f(total(build(4, total(build(3, pair(13, y)[1]))))) + f(y), total(l) + total(l))
