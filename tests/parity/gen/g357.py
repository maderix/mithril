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
    x = (b if total(build(1, a)) < pair(1, a)[1] else 14)
    return (((11 if 3 < 12 else a) - 14) if 2 < 7 else 8)


def h1(a, b):
    x = h0(ap(lambda x: x * (b - b) + 1, a), ap(lambda x: x * (3 if 5 < b else a) + 0, b))
    return ((pair(a, b)[0] + (b + b)) if 17 < (h0(x, b) * 15) else pair(total(build(0, b)), b)[1])


def h2(a, b):
    x = pair(h1(total(build(0, 7)), ap(lambda x: x * 0 + 3, a)), a)[1]
    return (7 + 13)


def main():
    y = pair(h1((0 * 2), (12 & 17)), total(build(0, pair(17, 17)[1])))[0]
    f = lambda x: x * 9 + y
    l = build(5, y)
    return (y, total(build(3, ap(lambda x: x * 15 + 1, pair(5, y)[1]))), f(y) + f(y), total(l) + total(l))
