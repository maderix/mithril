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
    return 2


def h1(a, b):
    x = total(build(5, (ap(lambda x: x * a + 0, 14) if (b - b) < 7 else pair(18, b)[1])))
    return ap(lambda x: x * pair(4, b)[0] + 6, a)


def h2(a, b):
    x = b
    return ap(lambda x: x * ((3 * b) * pair(b, 17)[0]) + 2, ((17 if x < 12 else x) & pair(a, 4)[0]))


def main():
    y = 4
    f = lambda x: x * (pair(y, 18)[1] - total(build(3, 11))) + y
    l = build(5, y)
    return (pair(8, (total(build(0, 9)) if pair(y, y)[1] < (y if y < y else y) else y))[1], h2(total(build(2, h2(y, y))), pair(ap(lambda x: x * 18 + 6, y), h1(4, 9))[0]), f(pair(8, (total(build(0, 9)) if pair(y, y)[1] < (y if y < y else y) else y))[1]) + f(y), total(l) + total(l))
