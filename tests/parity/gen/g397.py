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
    x = 8
    return ap(lambda x: x * (x if ap(lambda x: x * a + 4, 14) < (4 - b) else (8 + 3)) + 0, (b if ap(lambda x: x * 11 + 4, 8) < 15 else 10))


def h1(a, b):
    x = a
    return ((pair(x, 6)[0] if 4 < (x + b) else total(build(6, a))) if pair(x, total(build(1, 15)))[1] < (total(build(0, a)) if 15 < (a if 1 < x else x) else 5) else 15)


def h2(a, b):
    x = total(build(4, ap(lambda x: x * b + 7, (a if b < b else a))))
    return 19


def main():
    y = 15
    f = lambda x: x * ap(lambda x: x * (y if 18 < y else y) + 2, total(build(1, 19))) + y
    l = build(6, y)
    return (ap(lambda x: x * h2((y * 7), ap(lambda x: x * 14 + 1, y)) + 5, (pair(y, 7)[1] if ap(lambda x: x * 8 + 1, 17) < h1(y, y) else h1(16, y))), ap(lambda x: x * pair(total(build(4, 3)), pair(y, y)[1])[1] + 8, h0(h2(4, y), 10)), f(ap(lambda x: x * h2((y * 7), ap(lambda x: x * 14 + 1, y)) + 5, (pair(y, 7)[1] if ap(lambda x: x * 8 + 1, 17) < h1(y, y) else h1(16, y)))) + f(y), total(l) + total(l))
