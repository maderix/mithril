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
    x = total(build(2, ap(lambda x: x * a + 5, pair(a, a)[0])))
    return 4


def h1(a, b):
    x = (ap(lambda x: x * ap(lambda x: x * 9 + 3, a) + 4, pair(b, 12)[0]) ^ (h0(a, 9) * a))
    return total(build(5, 6))


def h2(a, b):
    x = ap(lambda x: x * 14 + 2, b)
    return (a ^ 12)


def main():
    y = 1
    f = lambda x: x * h0(total(build(0, y)), pair(16, 6)[1]) + y
    l = build(3, y)
    return (total(build(5, total(build(2, h0(7, 18))))), ap(lambda x: x * h0((4 - y), h1(10, y)) + 5, pair(h2(y, 15), 2)[1]), f(total(build(5, total(build(2, h0(7, 18)))))) + f(y), total(l) + total(l))
