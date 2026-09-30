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
    x = pair(total(build(0, 5)), 2)[0]
    return (total(build(6, ap(lambda x: x * 2 + 0, b))) ^ total(build(3, 16)))


def h1(a, b):
    x = ((total(build(2, 6)) if pair(11, b)[0] < (a if 0 < 2 else a) else ap(lambda x: x * 5 + 6, a)) if ap(lambda x: x * 3 + 8, (5 * b)) < h0((9 if b < 15 else 6), total(build(1, a))) else 11)
    return h0((pair(a, 12)[0] if (b ^ b) < a else h0(a, x)), ap(lambda x: x * h0(x, 6) + 4, h0(1, 18)))


def h2(a, b):
    x = ap(lambda x: x * (pair(b, b)[1] - h0(1, b)) + 3, total(build(0, a)))
    return pair(h0(pair(b, 19)[1], (x + b)), (b ^ ap(lambda x: x * 13 + 4, 2)))[0]


def main():
    y = ap(lambda x: x * ap(lambda x: x * 14 + 1, total(build(4, 6))) + 0, 5)
    f = lambda x: x * h0(pair(18, 2)[1], (y if y < 2 else y)) + y
    l = build(6, y)
    return (total(build(6, ((y - y) + ap(lambda x: x * y + 0, y)))), pair(ap(lambda x: x * pair(y, y)[0] + 0, (6 if y < 2 else 2)), (h0(2, y) * total(build(2, 16))))[1], f(total(build(6, ((y - y) + ap(lambda x: x * y + 0, y))))) + f(y), total(l) + total(l))
