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
    x = a
    return 16


def h1(a, b):
    x = pair(12, (ap(lambda x: x * b + 1, a) * a))[1]
    return ((3 if (14 if 14 < 12 else x) < (9 + 10) else (a & x)) & ap(lambda x: x * (15 if x < x else 9) + 7, b))


def h2(a, b):
    x = total(build(4, a))
    return (ap(lambda x: x * h0(b, 11) + 2, (11 * 18)) - ap(lambda x: x * (b ^ 4) + 7, ap(lambda x: x * x + 5, 17)))


def main():
    y = total(build(5, ((5 if 6 < 10 else 17) if (2 - 6) < pair(9, 0)[1] else pair(15, 8)[0])))
    f = lambda x: x * y + y
    l = build(5, y)
    return ((y ^ ap(lambda x: x * pair(y, y)[0] + 0, total(build(6, 5)))), pair(14, ap(lambda x: x * pair(13, 1)[0] + 7, (y & 4)))[1], f((y ^ ap(lambda x: x * pair(y, y)[0] + 0, total(build(6, 5))))) + f(y), total(l) + total(l))
