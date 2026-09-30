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
    x = (ap(lambda x: x * ap(lambda x: x * 3 + 2, b) + 1, total(build(1, a))) - ap(lambda x: x * b + 3, pair(a, b)[0]))
    return a


def h1(a, b):
    x = h0((total(build(6, a)) & (b & a)), pair(pair(19, 10)[0], total(build(1, b)))[1])
    return (8 if b < 5 else pair(pair(8, a)[0], ap(lambda x: x * 17 + 0, 10))[0])


def h2(a, b):
    x = b
    return (total(build(0, total(build(6, 1)))) if ap(lambda x: x * pair(3, a)[0] + 8, h1(8, a)) < (h0(a, 13) if total(build(3, 15)) < pair(x, a)[1] else total(build(3, b))) else a)


def main():
    y = 19
    f = lambda x: x * h1((y if 0 < 3 else y), 2) + y
    l = build(4, y)
    return (ap(lambda x: x * 4 + 1, h2((19 if 3 < y else 3), ap(lambda x: x * 8 + 1, y))), (19 if total(build(5, h1(12, y))) < 9 else total(build(0, y))), f(ap(lambda x: x * 4 + 1, h2((19 if 3 < y else 3), ap(lambda x: x * 8 + 1, y)))) + f(y), total(l) + total(l))
