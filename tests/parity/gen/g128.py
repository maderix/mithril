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
    x = 5
    return ap(lambda x: x * (pair(x, 10)[1] + pair(17, 1)[0]) + 1, (ap(lambda x: x * b + 8, b) if x < (17 + 1) else (14 if a < b else 3)))


def h1(a, b):
    x = ap(lambda x: x * total(build(6, h0(a, a))) + 5, ap(lambda x: x * total(build(6, a)) + 8, (b if a < a else 10)))
    return a


def h2(a, b):
    x = (h1(pair(a, a)[1], pair(15, 17)[0]) * (b * (a & b)))
    return (a if total(build(6, ap(lambda x: x * x + 1, 3))) < total(build(5, b)) else (total(build(0, 19)) & (1 & a)))


def main():
    y = (h1((10 if 17 < 16 else 6), 3) ^ (16 * total(build(6, 6))))
    f = lambda x: x * total(build(0, ap(lambda x: x * y + 3, 5))) + y
    l = build(1, y)
    return ((total(build(6, 4)) if pair((13 * 4), total(build(5, y)))[1] < (y - (y if 14 < 5 else y)) else ap(lambda x: x * y + 6, ap(lambda x: x * y + 3, y))), 15, f((total(build(6, 4)) if pair((13 * 4), total(build(5, y)))[1] < (y - (y if 14 < 5 else y)) else ap(lambda x: x * y + 6, ap(lambda x: x * y + 3, y)))) + f(y), total(l) + total(l))
