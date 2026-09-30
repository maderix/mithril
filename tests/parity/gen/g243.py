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
    x = 15
    return (ap(lambda x: x * 5 + 5, (b if 7 < 3 else a)) & 15)


def h1(a, b):
    x = h0(ap(lambda x: x * b + 4, total(build(4, a))), (a if pair(2, 6)[0] < ap(lambda x: x * b + 2, b) else b))
    return h0(5, total(build(6, ap(lambda x: x * 8 + 6, b))))


def h2(a, b):
    x = h1(total(build(5, (a if 2 < a else b))), total(build(6, total(build(3, 4)))))
    return (12 if total(build(0, pair(a, b)[1])) < ap(lambda x: x * (b + 7) + 1, h1(14, x)) else (pair(12, 4)[1] if pair(6, 15)[1] < total(build(5, 8)) else total(build(1, 3))))


def main():
    y = pair(h0(ap(lambda x: x * 18 + 2, 13), total(build(5, 4))), 19)[1]
    f = lambda x: x * 14 + y
    l = build(6, y)
    return ((y if y < total(build(3, (13 if 0 < 7 else 0))) else ((3 if y < 14 else y) if ap(lambda x: x * 6 + 6, 6) < 10 else total(build(0, 2)))), total(build(4, y)), f((y if y < total(build(3, (13 if 0 < 7 else 0))) else ((3 if y < 14 else y) if ap(lambda x: x * 6 + 6, 6) < 10 else total(build(0, 2))))) + f(y), total(l) + total(l))
