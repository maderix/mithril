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
    x = ap(lambda x: x * pair((11 ^ b), ap(lambda x: x * 8 + 0, 2))[1] + 0, (a if ap(lambda x: x * a + 7, a) < total(build(3, a)) else pair(1, b)[0]))
    return 0


def h1(a, b):
    x = ((ap(lambda x: x * 9 + 0, 13) & h0(a, 14)) + (b + 0))
    return pair(17, ap(lambda x: x * (x ^ x) + 2, pair(b, a)[1]))[1]


def h2(a, b):
    x = pair(1, ((a * b) if pair(16, 12)[1] < ap(lambda x: x * b + 6, a) else (a ^ b)))[0]
    return (total(build(5, total(build(0, 6)))) if x < ap(lambda x: x * pair(b, x)[0] + 5, pair(15, x)[0]) else pair(7, h0(16, 2))[1])


def main():
    y = total(build(2, h2(pair(9, 6)[1], pair(4, 17)[1])))
    f = lambda x: x * 13 + y
    l = build(6, y)
    return (h1(h0(17, y), y), y, f(h1(h0(17, y), y)) + f(y), total(l) + total(l))
