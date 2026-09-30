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
    x = (total(build(0, total(build(2, 15)))) * total(build(2, ap(lambda x: x * 11 + 0, b))))
    return b


def h1(a, b):
    x = ap(lambda x: x * pair(h0(6, 1), ap(lambda x: x * 5 + 1, 12))[0] + 6, h0(16, pair(7, 11)[1]))
    return 11


def h2(a, b):
    x = ap(lambda x: x * 14 + 2, ap(lambda x: x * ap(lambda x: x * a + 8, 8) + 8, 13))
    return pair(total(build(1, h0(b, x))), pair(h0(15, x), total(build(3, b)))[0])[0]


def main():
    y = ap(lambda x: x * (h2(17, 2) + (16 - 15)) + 0, 15)
    f = lambda x: x * pair(h1(8, y), (8 if y < 10 else 6))[0] + y
    l = build(4, y)
    return ((ap(lambda x: x * pair(1, 12)[1] + 3, ap(lambda x: x * 3 + 8, 10)) if pair(total(build(3, 7)), ap(lambda x: x * 9 + 4, 5))[0] < 13 else ap(lambda x: x * (3 if 7 < 0 else y) + 8, (y - 5))), (ap(lambda x: x * ap(lambda x: x * 8 + 6, y) + 3, ap(lambda x: x * 0 + 3, y)) if pair(total(build(3, 1)), y)[0] < pair(h0(y, 13), y)[1] else (total(build(1, y)) if ap(lambda x: x * y + 2, 9) < total(build(2, y)) else pair(9, 10)[1])), f((ap(lambda x: x * pair(1, 12)[1] + 3, ap(lambda x: x * 3 + 8, 10)) if pair(total(build(3, 7)), ap(lambda x: x * 9 + 4, 5))[0] < 13 else ap(lambda x: x * (3 if 7 < 0 else y) + 8, (y - 5)))) + f(y), total(l) + total(l))
