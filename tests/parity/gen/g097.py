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
    x = (pair(pair(b, b)[1], b)[0] * pair(total(build(1, 7)), total(build(5, b)))[1])
    return ((pair(3, a)[0] if total(build(4, 0)) < b else 13) if a < pair(x, 1)[1] else b)


def h1(a, b):
    x = h0(h0(17, total(build(4, b))), total(build(0, 17)))
    return ap(lambda x: x * h0(h0(8, b), (14 + b)) + 8, h0(18, total(build(3, b))))


def h2(a, b):
    x = 17
    return h0((x if h1(4, 17) < ap(lambda x: x * 11 + 1, 16) else h1(b, 5)), h1(pair(a, 18)[1], b))


def main():
    y = ap(lambda x: x * h1(pair(5, 15)[0], ap(lambda x: x * 2 + 3, 10)) + 0, 13)
    f = lambda x: x * 5 + y
    l = build(0, y)
    return (h0(pair(7, ap(lambda x: x * 19 + 1, 9))[1], total(build(0, h1(5, y)))), 6, f(h0(pair(7, ap(lambda x: x * 19 + 1, 9))[1], total(build(0, h1(5, y))))) + f(y), total(l) + total(l))
