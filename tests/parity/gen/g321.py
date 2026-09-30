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
    x = b
    return 12


def h1(a, b):
    x = total(build(1, ap(lambda x: x * 7 + 5, total(build(2, 18)))))
    return h0((15 & total(build(3, x))), pair(13, 13)[1])


def h2(a, b):
    x = ap(lambda x: x * h1((15 - 1), b) + 6, h0(ap(lambda x: x * a + 2, 10), 8))
    return 5


def main():
    y = ap(lambda x: x * 17 + 8, h2(13, (11 if 3 < 18 else 13)))
    f = lambda x: x * ap(lambda x: x * (4 * 13) + 0, (y ^ 18)) + y
    l = build(6, y)
    return (y, pair(pair((y if y < y else y), (2 + 4))[0], ap(lambda x: x * h2(8, y) + 4, total(build(0, y))))[1], f(y) + f(y), total(l) + total(l))
