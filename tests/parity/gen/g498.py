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
    x = (13 ^ (ap(lambda x: x * b + 5, b) if (8 if b < 18 else b) < 10 else (3 if 3 < 16 else 13)))
    return ((pair(7, x)[1] - 7) if 1 < 8 else 7)


def h1(a, b):
    x = (((b if 1 < a else b) if h0(a, b) < total(build(1, b)) else (9 if b < b else 9)) if 8 < ((b & 17) if ap(lambda x: x * a + 3, b) < pair(a, a)[0] else (b if a < b else b)) else 5)
    return h0(((x + 9) & total(build(0, b))), 12)


def h2(a, b):
    x = 16
    return ap(lambda x: x * (x if (14 if x < a else 2) < (a if a < 2 else 18) else (2 if a < b else x)) + 4, h0(b, (8 if b < 15 else 2)))


def main():
    y = pair(14, pair((3 if 7 < 18 else 5), 10)[1])[1]
    f = lambda x: x * h0(h2(y, y), (y if 0 < 16 else y)) + y
    l = build(2, y)
    return ((total(build(4, y)) - (ap(lambda x: x * y + 7, 2) & h1(y, y))), total(build(6, total(build(5, pair(y, y)[1])))), f((total(build(4, y)) - (ap(lambda x: x * y + 7, 2) & h1(y, y)))) + f(y), total(l) + total(l))
