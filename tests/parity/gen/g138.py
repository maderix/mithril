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
    x = ap(lambda x: x * b + 1, (ap(lambda x: x * 16 + 5, a) if 0 < (a if 12 < a else 1) else pair(2, 14)[0]))
    return pair(19, (ap(lambda x: x * b + 0, a) - x))[0]


def h1(a, b):
    x = b
    return pair(a, b)[0]


def h2(a, b):
    x = (((a & 5) if ap(lambda x: x * a + 6, a) < h0(15, 19) else (a if a < 16 else b)) & ap(lambda x: x * pair(14, a)[1] + 6, 11))
    return pair(total(build(0, ap(lambda x: x * a + 0, 2))), pair(total(build(4, b)), (b if 15 < 16 else 7))[0])[0]


def main():
    y = pair(9, total(build(3, (6 if 3 < 1 else 5))))[0]
    f = lambda x: x * (y - total(build(3, y))) + y
    l = build(6, y)
    return ((y ^ 1), 8, f((y ^ 1)) + f(y), total(l) + total(l))
