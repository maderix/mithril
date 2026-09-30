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
    x = 13
    return pair(11, 2)[1]


def h1(a, b):
    x = pair(total(build(1, b)), h0(13, ap(lambda x: x * a + 7, 7)))[1]
    return h0(h0(total(build(5, x)), (5 & 6)), pair((19 if b < b else 12), ap(lambda x: x * x + 4, a))[1])


def h2(a, b):
    x = h1(pair((17 if 7 < 0 else 5), 19)[1], pair((14 if a < 19 else 9), (11 - b))[1])
    return h0((h0(a, x) * total(build(4, a))), pair(9, pair(11, 6)[1])[0])


def main():
    y = (12 - 17)
    f = lambda x: x * (pair(13, 17)[0] if 8 < total(build(4, 8)) else total(build(6, y))) + y
    l = build(0, y)
    return (((y - total(build(6, y))) - total(build(3, pair(14, 8)[1]))), y, f(((y - total(build(6, y))) - total(build(3, pair(14, 8)[1])))) + f(y), total(l) + total(l))
