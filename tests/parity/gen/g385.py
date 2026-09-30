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
    x = (4 + ap(lambda x: x * 9 + 4, pair(10, 18)[0]))
    return 6


def h1(a, b):
    x = ap(lambda x: x * (h0(a, 15) + ap(lambda x: x * a + 6, b)) + 7, 19)
    return pair(19, 7)[0]


def h2(a, b):
    x = total(build(3, pair(ap(lambda x: x * b + 3, 4), ap(lambda x: x * b + 3, 9))[0]))
    return 18


def main():
    y = 17
    f = lambda x: x * pair(7, y)[1] + y
    l = build(5, y)
    return (y, (h2(y, y) * pair(ap(lambda x: x * 18 + 1, 6), h1(7, y))[0]), f(y) + f(y), total(l) + total(l))
