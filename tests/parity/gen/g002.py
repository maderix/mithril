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
    x = (ap(lambda x: x * 0 + 3, (3 ^ 14)) if ap(lambda x: x * 14 + 7, 9) < (13 if b < total(build(5, 4)) else (a if 18 < 10 else b)) else ap(lambda x: x * ap(lambda x: x * b + 4, 5) + 2, (b - 5)))
    return x


def h1(a, b):
    x = ap(lambda x: x * total(build(4, a)) + 2, total(build(2, 16)))
    return x


def h2(a, b):
    x = total(build(5, total(build(5, (9 - 16)))))
    return ap(lambda x: x * (pair(15, 4)[1] if h0(10, 18) < pair(16, a)[0] else 14) + 4, a)


def main():
    y = pair(total(build(5, (3 + 19))), ap(lambda x: x * h2(15, 11) + 5, 18))[1]
    f = lambda x: x * h1((15 if 1 < y else 8), ap(lambda x: x * 15 + 6, y)) + y
    l = build(3, y)
    return (pair(total(build(3, (18 - y))), total(build(2, y)))[0], (h1((y + 1), pair(4, 0)[1]) + 8), f(pair(total(build(3, (18 - y))), total(build(2, y)))[0]) + f(y), total(l) + total(l))
