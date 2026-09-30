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
    x = b
    return 11


def h1(a, b):
    x = (ap(lambda x: x * pair(13, 16)[0] + 4, h0(19, b)) if (h0(4, b) if pair(b, a)[1] < ap(lambda x: x * a + 8, a) else pair(b, 9)[1]) < (pair(9, a)[1] ^ (16 + b)) else ap(lambda x: x * 12 + 8, (2 if b < b else 0)))
    return h0(pair((a if 16 < a else 5), (4 + 2))[1], total(build(2, (a + 17))))


def h2(a, b):
    x = b
    return h1(total(build(0, total(build(1, x)))), (3 - ap(lambda x: x * x + 3, a)))


def main():
    y = ap(lambda x: x * ap(lambda x: x * h1(2, 2) + 7, pair(6, 3)[1]) + 3, h2((4 * 17), (10 ^ 8)))
    f = lambda x: x * total(build(6, (18 if y < y else 2))) + y
    l = build(3, y)
    return (((ap(lambda x: x * y + 8, y) + (5 & 7)) if y < total(build(0, (y if y < y else 6))) else 3), (y ^ ap(lambda x: x * ap(lambda x: x * 7 + 1, y) + 4, pair(y, 12)[1])), f(((ap(lambda x: x * y + 8, y) + (5 & 7)) if y < total(build(0, (y if y < y else 6))) else 3)) + f(y), total(l) + total(l))
