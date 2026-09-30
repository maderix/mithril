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
    x = 12
    return (5 if 11 < (total(build(3, 1)) * 10) else (pair(15, 13)[0] + 1))


def h1(a, b):
    x = total(build(2, ap(lambda x: x * (b ^ 6) + 4, ap(lambda x: x * a + 4, 10))))
    return pair(ap(lambda x: x * total(build(6, x)) + 7, a), (b ^ (1 & 14)))[1]


def h2(a, b):
    x = h0(((6 if 9 < 16 else 17) if h0(b, 15) < ap(lambda x: x * b + 6, a) else (b + 4)), a)
    return pair((19 ^ a), x)[1]


def main():
    y = h0(h2(total(build(3, 19)), (11 & 4)), pair((6 if 8 < 11 else 7), (6 - 14))[1])
    f = lambda x: x * pair(y, 0)[1] + y
    l = build(3, y)
    return (h2(1, h1(y, (y + 1))), ap(lambda x: x * ((4 - 7) if y < h2(y, y) else ap(lambda x: x * 14 + 8, 9)) + 6, h2((y if y < y else y), (6 if y < y else y))), f(h2(1, h1(y, (y + 1)))) + f(y), total(l) + total(l))
