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
    x = 4
    return 7


def h1(a, b):
    x = pair(ap(lambda x: x * pair(14, a)[0] + 5, ap(lambda x: x * a + 7, b)), 15)[0]
    return h0(pair(ap(lambda x: x * b + 2, 12), (16 + 9))[1], b)


def h2(a, b):
    x = (0 if pair(total(build(0, 4)), pair(6, 9)[1])[1] < h1(pair(a, 19)[0], (a if 4 < 12 else b)) else total(build(3, h0(11, a))))
    return a


def main():
    y = h2(pair(ap(lambda x: x * 0 + 0, 2), h0(16, 9))[0], pair(9, 16)[0])
    f = lambda x: x * y + y
    l = build(0, y)
    return (y, 19, f(y) + f(y), total(l) + total(l))
