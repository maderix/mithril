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
    x = ap(lambda x: x * 18 + 7, 19)
    return pair(b, ap(lambda x: x * (2 if 13 < x else b) + 6, (0 & x)))[0]


def h1(a, b):
    x = h0(h0(total(build(6, b)), b), total(build(6, b)))
    return pair(pair(ap(lambda x: x * 11 + 1, b), x)[1], ap(lambda x: x * h0(12, a) + 5, ap(lambda x: x * a + 4, 12)))[1]


def h2(a, b):
    x = total(build(1, a))
    return 1


def main():
    y = ap(lambda x: x * (total(build(2, 11)) ^ (7 if 9 < 16 else 18)) + 2, h2((19 ^ 11), h1(8, 7)))
    f = lambda x: x * (h1(y, y) - y) + y
    l = build(6, y)
    return ((total(build(0, pair(y, 5)[1])) if (h2(y, 2) ^ ap(lambda x: x * 14 + 1, y)) < ((y if y < y else 3) if y < total(build(5, 16)) else total(build(3, 5))) else h0(h2(3, 3), h1(14, y))), ap(lambda x: x * (16 + ap(lambda x: x * y + 7, y)) + 0, (total(build(1, y)) if total(build(6, 6)) < (4 + 14) else y)), f((total(build(0, pair(y, 5)[1])) if (h2(y, 2) ^ ap(lambda x: x * 14 + 1, y)) < ((y if y < y else 3) if y < total(build(5, 16)) else total(build(3, 5))) else h0(h2(3, 3), h1(14, y)))) + f(y), total(l) + total(l))
