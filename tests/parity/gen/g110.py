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
    x = ap(lambda x: x * a + 5, pair(10, total(build(1, 5)))[1])
    return (((b + 5) & total(build(2, 10))) if pair((b ^ b), 11)[1] < a else (ap(lambda x: x * x + 0, x) if 8 < ap(lambda x: x * 2 + 2, 14) else 5))


def h1(a, b):
    x = ap(lambda x: x * 12 + 3, (pair(a, 18)[1] if b < total(build(5, 6)) else (15 & b)))
    return total(build(3, (pair(15, 0)[1] & ap(lambda x: x * 3 + 0, 6))))


def h2(a, b):
    x = (pair(pair(a, a)[1], pair(3, a)[0])[1] if a < h0(pair(0, 16)[0], (3 * 8)) else 15)
    return h1(total(build(3, (6 * x))), total(build(1, (0 - 18))))


def main():
    y = pair(ap(lambda x: x * (10 ^ 11) + 7, pair(9, 2)[0]), ((11 if 3 < 16 else 15) ^ (6 if 7 < 7 else 5)))[0]
    f = lambda x: x * pair(y, total(build(6, y)))[1] + y
    l = build(2, y)
    return (ap(lambda x: x * ((y ^ 0) * total(build(6, 15))) + 6, h2((2 ^ y), h1(9, y))), 4, f(ap(lambda x: x * ((y ^ 0) * total(build(6, 15))) + 6, h2((2 ^ y), h1(9, y)))) + f(y), total(l) + total(l))
