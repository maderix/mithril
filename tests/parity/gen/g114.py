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
    x = b
    return ap(lambda x: x * total(build(4, pair(7, 7)[0])) + 3, 4)


def h1(a, b):
    x = pair(b, (pair(a, 15)[1] if a < (19 ^ a) else 15))[0]
    return pair(0, (total(build(5, x)) if (5 * x) < pair(x, 1)[0] else (b & a)))[0]


def h2(a, b):
    x = pair(pair(h1(b, b), ap(lambda x: x * 16 + 3, 3))[1], ap(lambda x: x * 12 + 6, pair(3, 2)[0]))[0]
    return ap(lambda x: x * pair((13 if x < b else a), pair(x, 7)[0])[0] + 1, (total(build(3, 15)) if b < (x + b) else 1))


def main():
    y = 0
    f = lambda x: x * total(build(5, y)) + y
    l = build(5, y)
    return ((total(build(1, total(build(4, 4)))) * 5), ((13 & (y if y < y else y)) ^ y), f((total(build(1, total(build(4, 4)))) * 5)) + f(y), total(l) + total(l))
