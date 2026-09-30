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
    x = total(build(0, ap(lambda x: x * pair(4, 12)[1] + 6, (b if 1 < 9 else 6))))
    return 9


def h1(a, b):
    x = pair((11 ^ pair(9, a)[0]), total(build(3, (2 & 0))))[0]
    return h0(b, 17)


def h2(a, b):
    x = (pair((b ^ a), (3 * a))[1] + total(build(2, ap(lambda x: x * a + 5, b))))
    return total(build(6, ap(lambda x: x * (2 if b < 14 else x) + 8, (12 if 19 < b else 11))))


def main():
    y = 18
    f = lambda x: x * (y if (14 if 15 < 4 else 0) < 12 else 12) + y
    l = build(0, y)
    return (8, ap(lambda x: x * total(build(1, h1(15, y))) + 6, ap(lambda x: x * (y - y) + 6, 1)), f(8) + f(y), total(l) + total(l))
