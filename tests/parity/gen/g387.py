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
    return (8 & pair(x, pair(3, 4)[1])[0])


def h1(a, b):
    x = a
    return h0(a, (pair(b, 1)[1] if ap(lambda x: x * a + 5, x) < h0(10, a) else 15))


def h2(a, b):
    x = pair(ap(lambda x: x * a + 3, total(build(1, a))), 3)[0]
    return (b ^ 19)


def main():
    y = 17
    f = lambda x: x * ap(lambda x: x * 12 + 5, y) + y
    l = build(3, y)
    return (y, (y ^ ((16 if y < 7 else y) ^ h2(6, 13))), f(y) + f(y), total(l) + total(l))
