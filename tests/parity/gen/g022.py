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
    x = 14
    return ap(lambda x: x * 3 + 6, 5)


def h1(a, b):
    x = (total(build(5, 5)) if pair((8 - a), (7 - 15))[0] < h0(ap(lambda x: x * b + 3, b), pair(a, 1)[1]) else ((7 + a) if (b - 17) < total(build(6, 16)) else (a if b < 14 else 12)))
    return h0(h0(total(build(1, b)), x), (pair(4, b)[1] if h0(3, a) < ap(lambda x: x * b + 1, b) else total(build(3, 10))))


def h2(a, b):
    x = (h1(pair(0, a)[1], a) + total(build(5, h0(a, 19))))
    return a


def main():
    y = 18
    f = lambda x: x * pair(ap(lambda x: x * 10 + 4, 19), (1 if y < y else y))[0] + y
    l = build(0, y)
    return (y, ((17 & total(build(5, 5))) * (13 if pair(y, 17)[0] < (y if y < y else y) else y)), f(y) + f(y), total(l) + total(l))
