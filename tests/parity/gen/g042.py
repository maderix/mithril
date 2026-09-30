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
    x = ap(lambda x: x * (ap(lambda x: x * b + 3, 7) if (18 if a < a else 13) < total(build(4, 3)) else 1) + 2, pair(ap(lambda x: x * 19 + 0, a), 2)[1])
    return total(build(1, total(build(6, (x * 2)))))


def h1(a, b):
    x = (h0(a, pair(b, 3)[1]) * pair((a & 18), 17)[1])
    return ap(lambda x: x * x + 2, pair(a, (a if a < b else b))[1])


def h2(a, b):
    x = h1(h0((13 if b < a else 14), total(build(2, 4))), ap(lambda x: x * (a if 13 < b else 7) + 6, h1(b, a)))
    return b


def main():
    y = total(build(4, (total(build(3, 4)) if ap(lambda x: x * 9 + 7, 1) < (14 - 18) else (19 + 2))))
    f = lambda x: x * h2(h2(13, y), total(build(5, y))) + y
    l = build(6, y)
    return (11, ap(lambda x: x * 11 + 0, h0(total(build(5, 8)), y)), f(11) + f(y), total(l) + total(l))
