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
    return 15


def h1(a, b):
    x = h0((h0(4, b) ^ (b & 11)), 4)
    return 17


def h2(a, b):
    x = b
    return total(build(6, ap(lambda x: x * total(build(4, 15)) + 0, pair(b, x)[0])))


def main():
    y = 0
    f = lambda x: x * h1((y if 0 < 5 else y), 11) + y
    l = build(1, y)
    return ((h0((11 * 3), y) & ap(lambda x: x * y + 6, y)), h1(((8 ^ y) if ap(lambda x: x * y + 0, y) < (y if 12 < y else 15) else h0(12, 9)), y), f((h0((11 * 3), y) & ap(lambda x: x * y + 6, y))) + f(y), total(l) + total(l))
