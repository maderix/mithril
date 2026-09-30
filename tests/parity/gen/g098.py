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
    x = (17 if (total(build(6, b)) if (9 if 19 < a else 19) < total(build(3, 17)) else (a ^ 11)) < ((b if b < a else 12) if pair(a, 9)[0] < total(build(3, a)) else (a if 8 < a else b)) else pair(pair(a, a)[1], 19)[1])
    return total(build(2, 18))


def h1(a, b):
    x = ((pair(5, 13)[1] + ap(lambda x: x * a + 5, b)) if ap(lambda x: x * (0 if b < b else 11) + 6, ap(lambda x: x * 9 + 8, 6)) < (h0(b, b) if 11 < h0(12, 9) else ap(lambda x: x * b + 0, a)) else total(build(5, (10 + b))))
    return (13 if 0 < h0(total(build(2, a)), total(build(0, 14))) else ap(lambda x: x * pair(a, b)[1] + 2, h0(4, 8)))


def h2(a, b):
    x = total(build(2, b))
    return ((15 - h1(5, x)) & h0(a, b))


def main():
    y = (1 - h0((16 - 1), 2))
    f = lambda x: x * h1(h1(0, y), (y + 4)) + y
    l = build(0, y)
    return (pair((h1(8, y) & ap(lambda x: x * 15 + 2, 11)), (y if (14 if 4 < y else y) < pair(y, y)[0] else ap(lambda x: x * y + 0, y)))[0], total(build(0, (ap(lambda x: x * 3 + 0, 1) if ap(lambda x: x * y + 4, 12) < total(build(1, y)) else pair(8, y)[0]))), f(pair((h1(8, y) & ap(lambda x: x * 15 + 2, 11)), (y if (14 if 4 < y else y) < pair(y, y)[0] else ap(lambda x: x * y + 0, y)))[0]) + f(y), total(l) + total(l))
