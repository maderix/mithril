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
    x = pair(total(build(1, (13 if 5 < b else 19))), (total(build(4, b)) if pair(b, a)[0] < 12 else ap(lambda x: x * 2 + 5, 8)))[1]
    return a


def h1(a, b):
    x = h0(((17 - a) if h0(7, 18) < b else b), (12 + total(build(6, 16))))
    return total(build(2, ap(lambda x: x * b + 6, (a if a < x else 6))))


def h2(a, b):
    x = (ap(lambda x: x * ap(lambda x: x * 4 + 1, a) + 1, ap(lambda x: x * 1 + 2, a)) + ap(lambda x: x * (a & a) + 7, ap(lambda x: x * 6 + 2, 4)))
    return (0 if 6 < (total(build(0, 12)) if x < pair(a, b)[0] else (6 if a < 4 else a)) else total(build(5, total(build(2, a)))))


def main():
    y = (12 if ap(lambda x: x * total(build(4, 12)) + 2, (16 if 0 < 6 else 11)) < 0 else ap(lambda x: x * (8 * 19) + 7, 15))
    f = lambda x: x * (total(build(1, 18)) ^ (y * 7)) + y
    l = build(5, y)
    return (y, (h0(h0(y, 14), pair(7, y)[0]) if h0(8, pair(4, y)[1]) < h0(pair(y, y)[0], pair(y, y)[1]) else total(build(5, total(build(0, y))))), f(y) + f(y), total(l) + total(l))
