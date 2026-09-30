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
    x = ((pair(7, b)[1] if (7 & b) < total(build(1, 11)) else 6) ^ ap(lambda x: x * total(build(2, a)) + 6, b))
    return 5


def h1(a, b):
    x = pair(h0(h0(9, 11), ap(lambda x: x * b + 6, a)), (h0(12, 16) & ap(lambda x: x * b + 8, b)))[0]
    return h0(19, pair(total(build(2, 9)), h0(a, b))[1])


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * 3 + 1, ap(lambda x: x * 11 + 5, a)) + 5, (b * (0 if 11 < a else b)))
    return total(build(6, ap(lambda x: x * total(build(6, 1)) + 2, (x if 8 < a else a))))


def main():
    y = 16
    f = lambda x: x * total(build(3, total(build(5, y)))) + y
    l = build(1, y)
    return (0, (5 if pair((9 + y), h1(y, y))[0] < pair((13 if 0 < 4 else 18), (15 if 5 < y else y))[1] else h0(ap(lambda x: x * y + 1, y), total(build(3, y)))), f(0) + f(y), total(l) + total(l))
