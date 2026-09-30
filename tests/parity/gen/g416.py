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
    x = ap(lambda x: x * pair((a if 17 < a else a), 3)[0] + 5, total(build(6, pair(11, a)[1])))
    return (((x + b) if pair(a, 9)[0] < ap(lambda x: x * 12 + 8, 6) else x) & pair((2 if 10 < 14 else b), 4)[0])


def h1(a, b):
    x = ((ap(lambda x: x * a + 3, a) ^ 10) if total(build(4, ap(lambda x: x * a + 3, a))) < h0(pair(3, 17)[0], (b if 14 < b else 4)) else 12)
    return total(build(4, ap(lambda x: x * h0(x, 9) + 1, ap(lambda x: x * 19 + 3, b))))


def h2(a, b):
    x = a
    return ap(lambda x: x * (ap(lambda x: x * b + 0, 10) if total(build(2, b)) < x else ap(lambda x: x * x + 8, b)) + 6, ((x if 12 < 18 else x) * total(build(4, x))))


def main():
    y = pair(h2(pair(2, 5)[0], 14), 15)[0]
    f = lambda x: x * pair(ap(lambda x: x * 3 + 2, y), (13 if y < 1 else 2))[0] + y
    l = build(5, y)
    return (ap(lambda x: x * 18 + 7, total(build(6, (1 ^ 12)))), ap(lambda x: x * y + 4, pair(pair(y, 8)[0], h0(4, y))[1]), f(ap(lambda x: x * 18 + 7, total(build(6, (1 ^ 12))))) + f(y), total(l) + total(l))
