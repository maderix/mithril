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
    x = pair((pair(14, 13)[0] ^ (16 if a < 13 else 6)), (total(build(2, b)) if 5 < (a + 19) else pair(b, 5)[1]))[1]
    return 9


def h1(a, b):
    x = total(build(4, (5 ^ (a * a))))
    return (7 + x)


def h2(a, b):
    x = total(build(2, ((2 if 7 < 7 else 1) * h1(b, 4))))
    return (((x if 19 < x else a) if ap(lambda x: x * a + 7, x) < x else (8 + a)) ^ ap(lambda x: x * 17 + 7, a))


def main():
    y = h0(15, 17)
    f = lambda x: x * total(build(0, ap(lambda x: x * 6 + 8, y))) + y
    l = build(3, y)
    return ((pair(ap(lambda x: x * 8 + 0, 9), h1(2, 5))[1] * y), pair(16, total(build(5, 9)))[0], f((pair(ap(lambda x: x * 8 + 0, 9), h1(2, 5))[1] * y)) + f(y), total(l) + total(l))
