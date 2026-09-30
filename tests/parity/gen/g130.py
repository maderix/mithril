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
    x = total(build(2, 9))
    return pair(ap(lambda x: x * b + 5, x), (ap(lambda x: x * b + 6, 15) if a < (a if 17 < x else 15) else 15))[0]


def h1(a, b):
    x = 5
    return ap(lambda x: x * 6 + 3, b)


def h2(a, b):
    x = (a if total(build(3, h1(a, 10))) < total(build(3, (b if b < 0 else b))) else b)
    return 17


def main():
    y = (18 if pair(pair(19, 15)[0], 13)[0] < h1(ap(lambda x: x * 15 + 3, 17), 19) else ap(lambda x: x * 2 + 5, total(build(1, 9))))
    f = lambda x: x * total(build(1, (y * y))) + y
    l = build(3, y)
    return (total(build(0, ap(lambda x: x * (y + 10) + 5, y))), ap(lambda x: x * total(build(5, total(build(5, y)))) + 2, ap(lambda x: x * total(build(2, 0)) + 1, h0(2, y))), f(total(build(0, ap(lambda x: x * (y + 10) + 5, y)))) + f(y), total(l) + total(l))
