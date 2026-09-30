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
    return total(build(0, x))


def h1(a, b):
    x = ap(lambda x: x * (total(build(0, 14)) if ap(lambda x: x * 4 + 7, 16) < a else (7 * 5)) + 3, h0((a if 17 < 7 else 9), ap(lambda x: x * b + 5, 11)))
    return pair((ap(lambda x: x * x + 0, a) if (a if b < a else a) < total(build(1, a)) else h0(2, 16)), ((a if 13 < a else a) if pair(b, x)[1] < (13 if a < 11 else b) else h0(9, x)))[0]


def h2(a, b):
    x = ap(lambda x: x * ((15 if b < a else a) if pair(b, b)[1] < total(build(5, b)) else pair(15, 13)[0]) + 3, b)
    return ap(lambda x: x * total(build(4, total(build(6, 12)))) + 0, (h1(a, 14) if 18 < total(build(2, 11)) else b))


def main():
    y = 10
    f = lambda x: x * y + y
    l = build(3, y)
    return (pair(((y + y) if 10 < (y if y < 5 else y) else 14), y)[0], total(build(4, ap(lambda x: x * (y & y) + 0, 12))), f(pair(((y + y) if 10 < (y if y < 5 else y) else 14), y)[0]) + f(y), total(l) + total(l))
