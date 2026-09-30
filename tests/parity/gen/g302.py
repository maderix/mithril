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
    x = total(build(1, b))
    return (4 if x < (a if a < b else (18 if a < x else 4)) else a)


def h1(a, b):
    x = b
    return pair(pair(total(build(3, a)), x)[0], 7)[0]


def h2(a, b):
    x = (ap(lambda x: x * h0(a, a) + 2, pair(19, a)[1]) * (a ^ 14))
    return ap(lambda x: x * ap(lambda x: x * (14 * a) + 2, total(build(2, 7))) + 2, ap(lambda x: x * total(build(5, a)) + 3, total(build(3, 18))))


def main():
    y = (18 ^ (pair(1, 15)[1] * h0(11, 14)))
    f = lambda x: x * ap(lambda x: x * 14 + 8, (14 + 13)) + y
    l = build(6, y)
    return (y, total(build(4, ((y ^ 10) ^ (9 - y)))), f(y) + f(y), total(l) + total(l))
