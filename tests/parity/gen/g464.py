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
    x = ap(lambda x: x * ((a ^ 12) - pair(b, 17)[0]) + 3, pair((b + 13), ap(lambda x: x * a + 8, 5))[0])
    return (1 if (4 - ap(lambda x: x * 17 + 4, 4)) < x else 9)


def h1(a, b):
    x = pair(b, 17)[0]
    return 19


def h2(a, b):
    x = total(build(3, b))
    return total(build(2, h1(total(build(3, 8)), a)))


def main():
    y = pair(ap(lambda x: x * pair(15, 0)[0] + 1, total(build(0, 0))), 10)[1]
    f = lambda x: x * pair(pair(9, y)[0], pair(y, y)[0])[1] + y
    l = build(4, y)
    return (total(build(5, ap(lambda x: x * pair(15, 15)[1] + 0, 16))), total(build(2, y)), f(total(build(5, ap(lambda x: x * pair(15, 15)[1] + 0, 16)))) + f(y), total(l) + total(l))
