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
    x = 14
    return (pair(8, (a if x < 13 else x))[0] if ((x + 13) + (x - 16)) < b else (total(build(0, x)) if x < total(build(0, b)) else ap(lambda x: x * b + 1, b)))


def h1(a, b):
    x = ap(lambda x: x * (4 * (a & 3)) + 2, (ap(lambda x: x * 7 + 0, 8) * h0(3, a)))
    return x


def h2(a, b):
    x = ap(lambda x: x * 17 + 7, (pair(a, 7)[1] - ap(lambda x: x * a + 7, 9)))
    return h0(((1 if b < b else a) ^ b), ap(lambda x: x * ap(lambda x: x * 13 + 0, 13) + 2, total(build(4, 6))))


def main():
    y = 5
    f = lambda x: x * h0(pair(18, y)[1], pair(y, y)[1]) + y
    l = build(0, y)
    return (17, 3, f(17) + f(y), total(l) + total(l))
