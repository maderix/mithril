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
    x = ((13 if pair(8, b)[1] < (a if 19 < 3 else 9) else a) & ap(lambda x: x * (b if 2 < 12 else 12) + 5, pair(17, 8)[0]))
    return 18


def h1(a, b):
    x = total(build(6, total(build(2, ap(lambda x: x * 8 + 4, b)))))
    return b


def h2(a, b):
    x = (h0(pair(17, 4)[1], pair(12, a)[0]) if ap(lambda x: x * h1(1, 4) + 2, pair(a, 16)[0]) < total(build(1, a)) else h0(a, a))
    return ((total(build(5, 13)) if total(build(1, a)) < 17 else pair(x, 7)[0]) * ap(lambda x: x * total(build(2, 16)) + 4, a))


def main():
    y = ap(lambda x: x * h0(8, (6 if 3 < 3 else 10)) + 4, total(build(3, 17)))
    f = lambda x: x * 5 + y
    l = build(3, y)
    return (pair(ap(lambda x: x * pair(y, y)[1] + 4, h2(2, y)), h1(ap(lambda x: x * y + 0, 0), (19 if y < y else y)))[0], (y - 1), f(pair(ap(lambda x: x * pair(y, y)[1] + 4, h2(2, y)), h1(ap(lambda x: x * y + 0, 0), (19 if y < y else y)))[0]) + f(y), total(l) + total(l))
