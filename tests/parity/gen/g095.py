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
    return b


def h1(a, b):
    x = (a if b < ((7 + 14) if 11 < pair(13, 17)[0] else ap(lambda x: x * 16 + 2, 14)) else (total(build(0, b)) if b < (a if b < b else 4) else b))
    return total(build(1, x))


def h2(a, b):
    x = total(build(1, (total(build(4, 2)) if b < total(build(1, 12)) else 12)))
    return 11


def main():
    y = h2(ap(lambda x: x * h1(16, 12) + 2, 15), pair(h1(17, 7), pair(18, 13)[1])[0])
    f = lambda x: x * pair((y ^ 18), ap(lambda x: x * 18 + 6, y))[0] + y
    l = build(4, y)
    return ((y ^ pair((y + y), ap(lambda x: x * y + 6, 15))[0]), ap(lambda x: x * (16 if ap(lambda x: x * y + 1, 0) < 15 else total(build(6, 9))) + 5, pair((6 if y < y else y), pair(y, 3)[1])[1]), f((y ^ pair((y + y), ap(lambda x: x * y + 6, 15))[0])) + f(y), total(l) + total(l))
