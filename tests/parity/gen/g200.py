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
    x = a
    return ((ap(lambda x: x * b + 3, 12) if pair(x, 13)[0] < (6 if 14 < 3 else 19) else total(build(6, a))) if 3 < pair(pair(14, 19)[1], 15)[0] else total(build(3, a)))


def h1(a, b):
    x = (9 if ((13 & 15) if total(build(6, a)) < (a * b) else total(build(0, a))) < (b & ap(lambda x: x * a + 4, b)) else total(build(1, total(build(3, a)))))
    return ((total(build(5, a)) if total(build(5, b)) < total(build(3, 19)) else a) if pair(ap(lambda x: x * 10 + 1, b), 19)[1] < ap(lambda x: x * pair(a, 2)[0] + 1, 12) else (ap(lambda x: x * x + 3, x) & h0(7, x)))


def h2(a, b):
    x = total(build(3, b))
    return x


def main():
    y = 9
    f = lambda x: x * 16 + y
    l = build(4, y)
    return (pair(pair(h0(6, 4), (0 & 10))[1], ap(lambda x: x * pair(y, 3)[0] + 6, total(build(5, 6))))[0], total(build(2, total(build(6, (19 + 2))))), f(pair(pair(h0(6, 4), (0 & 10))[1], ap(lambda x: x * pair(y, 3)[0] + 6, total(build(5, 6))))[0]) + f(y), total(l) + total(l))
