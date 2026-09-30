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
    x = 18
    return (ap(lambda x: x * x + 3, (8 ^ 1)) if (x - 11) < 15 else 9)


def h1(a, b):
    x = h0((16 if 8 < ap(lambda x: x * b + 4, 6) else a), pair(6, (b - 13))[0])
    return (pair(h0(14, 1), a)[1] & ap(lambda x: x * a + 4, total(build(3, a))))


def h2(a, b):
    x = pair(1, (b ^ b))[1]
    return ap(lambda x: x * pair(pair(17, a)[0], (x if b < x else 13))[0] + 0, x)


def main():
    y = 9
    f = lambda x: x * h1((19 * y), ap(lambda x: x * 14 + 7, 11)) + y
    l = build(5, y)
    return ((ap(lambda x: x * 14 + 7, ap(lambda x: x * 17 + 1, y)) * y), (y if pair((0 if y < 11 else 15), (y ^ 2))[1] < (y - (2 if y < y else 0)) else y), f((ap(lambda x: x * 14 + 7, ap(lambda x: x * 17 + 1, y)) * y)) + f(y), total(l) + total(l))
