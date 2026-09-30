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
    x = 9
    return pair(ap(lambda x: x * 0 + 8, (a - b)), ((b * 13) if (15 & a) < 18 else 9))[0]


def h1(a, b):
    x = ap(lambda x: x * (10 & h0(a, b)) + 2, total(build(2, (a * 13))))
    return pair(0, pair(total(build(6, b)), (b if 16 < a else 5))[1])[1]


def h2(a, b):
    x = (h0((10 if 12 < 19 else a), ap(lambda x: x * 7 + 3, a)) if b < pair(h1(b, 2), (6 + a))[0] else 3)
    return ((total(build(2, b)) if h1(b, 1) < (x ^ a) else (a if b < 16 else 11)) if (ap(lambda x: x * x + 2, 2) * (9 & 13)) < a else x)


def main():
    y = total(build(4, total(build(5, (8 + 11)))))
    f = lambda x: x * (pair(0, y)[1] + total(build(5, y))) + y
    l = build(0, y)
    return (pair(h1(total(build(0, 18)), total(build(3, 8))), (pair(12, y)[0] + total(build(2, 15))))[1], y, f(pair(h1(total(build(0, 18)), total(build(3, 8))), (pair(12, y)[0] + total(build(2, 15))))[1]) + f(y), total(l) + total(l))
