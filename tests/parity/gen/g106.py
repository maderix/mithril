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
    x = (b - pair(19, 2)[0])
    return 17


def h1(a, b):
    x = ap(lambda x: x * (pair(b, b)[1] if pair(10, 3)[0] < ap(lambda x: x * 17 + 1, b) else a) + 7, ap(lambda x: x * b + 2, (10 if 11 < 14 else a)))
    return b


def h2(a, b):
    x = ((pair(b, a)[1] if 14 < pair(17, b)[0] else 8) if pair(a, 18)[0] < 12 else ((a & b) if b < (15 if 15 < a else 7) else total(build(0, 19))))
    return total(build(6, total(build(1, (a - x)))))


def main():
    y = ap(lambda x: x * 12 + 2, (h0(6, 3) if pair(15, 18)[0] < (10 if 10 < 9 else 3) else 19))
    f = lambda x: x * (0 & (y ^ y)) + y
    l = build(2, y)
    return ((pair(y, 6)[0] ^ (pair(y, y)[1] ^ ap(lambda x: x * y + 3, 8))), (y if ap(lambda x: x * pair(16, 2)[0] + 4, total(build(5, 16))) < (h1(y, 1) - 13) else 0), f((pair(y, 6)[0] ^ (pair(y, y)[1] ^ ap(lambda x: x * y + 3, 8)))) + f(y), total(l) + total(l))
