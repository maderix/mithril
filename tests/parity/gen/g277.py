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
    return a


def h1(a, b):
    x = b
    return total(build(5, ((1 - 17) - total(build(5, b)))))


def h2(a, b):
    x = (total(build(5, total(build(3, 2)))) if pair((b if a < 4 else b), pair(b, 14)[1])[0] < h0(b, (16 if 4 < a else b)) else 0)
    return h1(((12 & 15) if b < h0(9, b) else 8), (ap(lambda x: x * x + 5, x) - b))


def main():
    y = (h2(pair(7, 13)[0], (15 if 16 < 12 else 9)) if ap(lambda x: x * 7 + 8, ap(lambda x: x * 13 + 3, 6)) < 1 else (ap(lambda x: x * 7 + 7, 9) - ap(lambda x: x * 1 + 1, 9)))
    f = lambda x: x * y + y
    l = build(3, y)
    return ((h0(pair(13, 10)[1], 4) if y < h1(h0(y, 17), 19) else (y + pair(y, 7)[0])), h2(ap(lambda x: x * h1(y, y) + 1, 2), 11), f((h0(pair(13, 10)[1], 4) if y < h1(h0(y, 17), 19) else (y + pair(y, 7)[0]))) + f(y), total(l) + total(l))
