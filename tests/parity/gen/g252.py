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
    x = 11
    return total(build(3, 18))


def h1(a, b):
    x = pair(14, 14)[0]
    return a


def h2(a, b):
    x = ap(lambda x: x * ap(lambda x: x * (b if 15 < a else a) + 7, (a if 14 < b else 3)) + 0, 15)
    return total(build(5, 16))


def main():
    y = (total(build(6, pair(7, 7)[1])) if pair((12 ^ 1), (9 if 12 < 10 else 8))[1] < 4 else 15)
    f = lambda x: x * ((y ^ 17) + total(build(1, y))) + y
    l = build(0, y)
    return (ap(lambda x: x * total(build(5, y)) + 8, total(build(5, total(build(3, 1))))), (ap(lambda x: x * y + 5, h1(4, y)) * pair(h1(y, y), h1(y, y))[1]), f(ap(lambda x: x * total(build(5, y)) + 8, total(build(5, total(build(3, 1)))))) + f(y), total(l) + total(l))
