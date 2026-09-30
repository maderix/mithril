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
    x = (a ^ b)
    return 1


def h1(a, b):
    x = 1
    return b


def h2(a, b):
    x = pair(((1 if b < 9 else a) if (10 if 7 < 2 else 8) < (b & b) else ap(lambda x: x * a + 5, b)), total(build(6, b)))[1]
    return total(build(1, pair((0 if x < x else a), h1(8, 6))[0]))


def main():
    y = (h1((7 if 9 < 14 else 11), h1(16, 19)) + (ap(lambda x: x * 17 + 4, 18) - ap(lambda x: x * 5 + 6, 12)))
    f = lambda x: x * h0(ap(lambda x: x * 16 + 7, y), total(build(6, y))) + y
    l = build(0, y)
    return (ap(lambda x: x * (13 if 1 < (y if 2 < y else y) else h1(13, y)) + 3, ((13 if y < 19 else y) if pair(y, y)[0] < 9 else 0)), y, f(ap(lambda x: x * (13 if 1 < (y if 2 < y else y) else h1(13, y)) + 3, ((13 if y < 19 else y) if pair(y, y)[0] < 9 else 0))) + f(y), total(l) + total(l))
