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
    x = pair(ap(lambda x: x * (a if b < b else 1) + 4, total(build(3, 5))), 15)[0]
    return ap(lambda x: x * 8 + 2, 17)


def h1(a, b):
    x = (b if h0(19, (0 * 3)) < 19 else h0(pair(16, 19)[1], pair(a, a)[0]))
    return pair(ap(lambda x: x * (12 - x) + 3, (x if a < b else a)), a)[1]


def h2(a, b):
    x = h0(h1((a ^ 3), (2 if 6 < 1 else 6)), h0(a, (0 if b < a else a)))
    return ap(lambda x: x * h1(ap(lambda x: x * 16 + 7, b), total(build(4, b))) + 6, x)


def main():
    y = pair(total(build(2, (7 * 4))), total(build(3, (12 if 13 < 12 else 5))))[0]
    f = lambda x: x * total(build(5, pair(18, y)[1])) + y
    l = build(6, y)
    return (y, (h0((7 if y < y else 12), ap(lambda x: x * y + 0, 5)) if y < 3 else y), f(y) + f(y), total(l) + total(l))
