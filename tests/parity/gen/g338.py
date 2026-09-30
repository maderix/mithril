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
    x = 17
    return x


def h1(a, b):
    x = (b * (total(build(1, b)) + h0(a, 14)))
    return ((ap(lambda x: x * x + 6, 13) if 6 < ap(lambda x: x * b + 2, x) else ap(lambda x: x * 13 + 5, x)) if (h0(16, 4) & pair(18, 5)[1]) < ap(lambda x: x * 4 + 4, total(build(4, 15))) else ap(lambda x: x * h0(19, 6) + 7, ap(lambda x: x * a + 5, 9)))


def h2(a, b):
    x = b
    return 5


def main():
    y = h2(17, pair(17, (18 if 3 < 7 else 17))[1])
    f = lambda x: x * pair(total(build(3, y)), total(build(5, 17)))[1] + y
    l = build(2, y)
    return ((h1(19, (y if y < 6 else 15)) - ap(lambda x: x * ap(lambda x: x * y + 3, 12) + 8, (7 if 5 < 5 else 15))), total(build(6, y)), f((h1(19, (y if y < 6 else 15)) - ap(lambda x: x * ap(lambda x: x * y + 3, 12) + 8, (7 if 5 < 5 else 15)))) + f(y), total(l) + total(l))
