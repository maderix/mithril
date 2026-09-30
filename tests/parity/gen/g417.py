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
    x = b
    return (11 if pair((a & b), total(build(5, x)))[0] < ap(lambda x: x * (x + a) + 5, a) else pair(pair(b, b)[1], (14 if b < 9 else b))[1])


def h1(a, b):
    x = total(build(5, 0))
    return (a & ap(lambda x: x * pair(a, b)[0] + 0, h0(12, a)))


def h2(a, b):
    x = h0(17, h1((b & 18), total(build(4, 7))))
    return (x if (total(build(5, 18)) + h1(8, b)) < h1(pair(a, 6)[0], (7 - 19)) else b)


def main():
    y = h0((0 + ap(lambda x: x * 0 + 1, 10)), (ap(lambda x: x * 0 + 7, 19) * (10 if 0 < 6 else 19)))
    f = lambda x: x * h2(pair(13, y)[0], 17) + y
    l = build(5, y)
    return (5, h2((y if 1 < ap(lambda x: x * 17 + 3, 11) else (y if y < 1 else y)), (pair(12, 0)[1] * pair(5, 15)[0])), f(5) + f(y), total(l) + total(l))
