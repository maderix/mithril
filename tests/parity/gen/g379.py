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
    return ap(lambda x: x * 10 + 1, total(build(4, (13 if 3 < 9 else x))))


def h1(a, b):
    x = b
    return ap(lambda x: x * pair(ap(lambda x: x * 12 + 8, 8), b)[1] + 7, (pair(x, a)[0] ^ (b ^ 13)))


def h2(a, b):
    x = (5 + ((a if a < 4 else 7) * (4 if 11 < b else 8)))
    return 17


def main():
    y = total(build(0, (13 ^ total(build(1, 3)))))
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * y + 4, 4) + 4, (y ^ y)) + y
    l = build(1, y)
    return (ap(lambda x: x * pair(total(build(4, 7)), y)[0] + 5, (total(build(1, y)) + (y + 12))), h2(pair(pair(y, y)[0], (1 if 14 < 4 else 7))[0], ap(lambda x: x * ap(lambda x: x * 19 + 0, y) + 2, 8)), f(ap(lambda x: x * pair(total(build(4, 7)), y)[0] + 5, (total(build(1, y)) + (y + 12)))) + f(y), total(l) + total(l))
