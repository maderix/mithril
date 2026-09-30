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
    x = 19
    return 6


def h1(a, b):
    x = 18
    return total(build(4, pair(ap(lambda x: x * 1 + 6, 13), total(build(3, x)))[0]))


def h2(a, b):
    x = h1(ap(lambda x: x * h0(3, a) + 3, ap(lambda x: x * b + 4, b)), pair(ap(lambda x: x * 12 + 4, a), a)[0])
    return ap(lambda x: x * ((0 if b < x else 19) ^ total(build(6, 5))) + 5, ((a * 4) - 8))


def main():
    y = 17
    f = lambda x: x * pair(h0(2, y), pair(y, 0)[0])[0] + y
    l = build(6, y)
    return (total(build(1, y)), (y ^ h2(total(build(0, 5)), total(build(3, 1)))), f(total(build(1, y))) + f(y), total(l) + total(l))
