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
    x = 13
    return 2


def h1(a, b):
    x = b
    return (pair(total(build(1, a)), pair(9, a)[0])[0] if (ap(lambda x: x * 3 + 8, 2) * total(build(0, 16))) < ((a & x) & 18) else h0(b, ap(lambda x: x * x + 7, 14)))


def h2(a, b):
    x = (ap(lambda x: x * (6 + a) + 8, b) ^ pair((b if 11 < 8 else 5), ap(lambda x: x * a + 3, a))[0])
    return ap(lambda x: x * 19 + 7, ((b ^ 3) if b < (a if 9 < 15 else b) else ap(lambda x: x * 0 + 1, b)))


def main():
    y = 1
    f = lambda x: x * total(build(5, (y if y < 10 else 19))) + y
    l = build(2, y)
    return (ap(lambda x: x * h2(7, (y * y)) + 6, y), pair(10, y)[1], f(ap(lambda x: x * h2(7, (y * y)) + 6, y)) + f(y), total(l) + total(l))
