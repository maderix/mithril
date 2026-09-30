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
    x = 1
    return (3 * (total(build(6, b)) - ap(lambda x: x * a + 0, 9)))


def h1(a, b):
    x = total(build(0, (pair(3, 17)[1] * pair(0, 13)[0])))
    return 7


def h2(a, b):
    x = (total(build(0, total(build(1, 4)))) if (total(build(0, b)) if h1(b, 13) < 10 else ap(lambda x: x * 18 + 8, 2)) < pair(h0(17, 8), pair(a, 1)[0])[1] else pair(h0(a, 0), h0(a, 1))[0])
    return a


def main():
    y = pair((total(build(2, 7)) if h2(3, 2) < total(build(2, 3)) else total(build(6, 10))), ap(lambda x: x * (5 + 4) + 4, pair(7, 3)[0]))[1]
    f = lambda x: x * (h1(y, 8) if 2 < total(build(3, y)) else h2(y, y)) + y
    l = build(6, y)
    return (((total(build(4, 16)) if total(build(5, y)) < y else 13) + y), pair(total(build(5, pair(y, y)[1])), y)[0], f(((total(build(4, 16)) if total(build(5, y)) < y else 13) + y)) + f(y), total(l) + total(l))
