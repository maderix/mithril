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
    x = 9
    return ap(lambda x: x * total(build(4, pair(13, 13)[1])) + 6, ((8 ^ 15) & (18 + 6)))


def h1(a, b):
    x = h0((ap(lambda x: x * a + 4, 17) if (8 * b) < (11 if 15 < 13 else 9) else pair(4, a)[0]), a)
    return pair((h0(1, 0) if x < (11 if b < 16 else 10) else pair(2, 18)[1]), ap(lambda x: x * 18 + 7, (b & a)))[1]


def h2(a, b):
    x = a
    return (6 + pair(ap(lambda x: x * 11 + 3, 1), h0(a, 10))[0])


def main():
    y = ap(lambda x: x * ((18 * 15) if ap(lambda x: x * 13 + 3, 9) < h2(10, 12) else pair(15, 13)[1]) + 7, h1(h1(13, 1), (6 if 5 < 3 else 13)))
    f = lambda x: x * ((y if 16 < y else 6) if (0 if 19 < 8 else y) < 19 else total(build(4, 12))) + y
    l = build(0, y)
    return (y, h1(4, y), f(y) + f(y), total(l) + total(l))
