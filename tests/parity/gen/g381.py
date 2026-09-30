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
    x = pair(((10 if a < a else a) if 2 < b else (a + b)), (ap(lambda x: x * 17 + 3, 9) + (19 - b)))[0]
    return pair(b, 5)[0]


def h1(a, b):
    x = (6 if pair(a, 9)[0] < (pair(a, b)[1] if ap(lambda x: x * 18 + 6, a) < total(build(2, 12)) else b) else h0(ap(lambda x: x * a + 7, 3), (13 * 0)))
    return (13 & pair(x, 14)[1])


def h2(a, b):
    x = ap(lambda x: x * h0(1, h0(10, 16)) + 6, (total(build(4, a)) ^ ap(lambda x: x * 2 + 1, a)))
    return a


def main():
    y = total(build(6, h0(h2(13, 13), (16 if 13 < 14 else 19))))
    f = lambda x: x * 6 + y
    l = build(1, y)
    return (total(build(3, 16)), pair(1, ap(lambda x: x * ap(lambda x: x * 14 + 1, y) + 4, 8))[0], f(total(build(3, 16))) + f(y), total(l) + total(l))
