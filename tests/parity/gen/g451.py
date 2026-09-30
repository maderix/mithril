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
    x = total(build(4, ap(lambda x: x * (a & 19) + 1, b)))
    return ((ap(lambda x: x * a + 5, a) if (3 if a < 3 else b) < 6 else total(build(3, x))) - (8 + 5))


def h1(a, b):
    x = h0(b, total(build(5, total(build(1, a)))))
    return total(build(0, a))


def h2(a, b):
    x = ap(lambda x: x * pair(h0(b, 13), 13)[0] + 6, h1(0, pair(b, a)[0]))
    return ((pair(12, 10)[1] if 13 < ap(lambda x: x * b + 8, a) else 0) if total(build(3, pair(b, 7)[1])) < 11 else x)


def main():
    y = ((total(build(0, 19)) & total(build(3, 11))) * total(build(4, (7 ^ 2))))
    f = lambda x: x * total(build(2, 4)) + y
    l = build(2, y)
    return (ap(lambda x: x * ap(lambda x: x * h0(y, y) + 1, 2) + 3, y), pair(((y * y) if y < pair(0, y)[0] else (6 ^ y)), total(build(5, total(build(0, 8)))))[0], f(ap(lambda x: x * ap(lambda x: x * h0(y, y) + 1, 2) + 3, y)) + f(y), total(l) + total(l))
