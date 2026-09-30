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
    x = 2
    return ap(lambda x: x * 17 + 8, (6 * pair(a, 13)[1]))


def h1(a, b):
    x = 11
    return total(build(6, 19))


def h2(a, b):
    x = b
    return total(build(2, a))


def main():
    y = h2(((5 - 5) + ap(lambda x: x * 18 + 2, 3)), 18)
    f = lambda x: x * pair((8 ^ y), total(build(1, y)))[1] + y
    l = build(5, y)
    return (h0(h0((13 + 7), (y * 11)), pair(ap(lambda x: x * y + 2, y), (y if y < 8 else 14))[1]), (((y - 9) if 19 < 12 else (16 if y < 6 else 19)) * (ap(lambda x: x * 9 + 5, 17) - total(build(3, y)))), f(h0(h0((13 + 7), (y * 11)), pair(ap(lambda x: x * y + 2, y), (y if y < 8 else 14))[1])) + f(y), total(l) + total(l))
