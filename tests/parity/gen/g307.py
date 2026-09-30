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
    x = ap(lambda x: x * (15 if (4 if a < b else a) < a else ap(lambda x: x * b + 8, 4)) + 3, a)
    return pair((ap(lambda x: x * b + 7, 0) & total(build(6, x))), pair(x, total(build(0, 15)))[1])[0]


def h1(a, b):
    x = ((total(build(4, a)) if (a if b < 2 else b) < a else a) if pair(h0(1, 8), (a & a))[0] < 2 else total(build(4, (a - 6))))
    return (18 * ap(lambda x: x * (10 ^ a) + 0, (a - 9)))


def h2(a, b):
    x = (pair(ap(lambda x: x * b + 2, a), (6 - a))[0] if b < (total(build(5, b)) * h1(1, b)) else h1(total(build(2, 9)), ap(lambda x: x * a + 6, 7)))
    return total(build(0, x))


def main():
    y = 19
    f = lambda x: x * h2((y if 7 < 8 else 10), y) + y
    l = build(6, y)
    return (ap(lambda x: x * total(build(2, pair(6, 19)[1])) + 1, ap(lambda x: x * total(build(3, y)) + 0, (14 ^ 0))), (total(build(5, (y * 18))) if total(build(4, h2(y, y))) < total(build(3, ap(lambda x: x * y + 0, 10))) else y), f(ap(lambda x: x * total(build(2, pair(6, 19)[1])) + 1, ap(lambda x: x * total(build(3, y)) + 0, (14 ^ 0)))) + f(y), total(l) + total(l))
