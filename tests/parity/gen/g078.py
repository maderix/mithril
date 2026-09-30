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
    x = ((b if (b ^ 13) < ap(lambda x: x * b + 5, a) else 3) if pair(pair(17, b)[1], total(build(2, 3)))[0] < ap(lambda x: x * (b + a) + 2, 5) else 18)
    return ap(lambda x: x * total(build(6, pair(a, 7)[0])) + 7, 1)


def h1(a, b):
    x = total(build(0, (total(build(4, 11)) - total(build(2, a)))))
    return total(build(6, pair(8, ap(lambda x: x * 11 + 7, x))[0]))


def h2(a, b):
    x = (b if 6 < 13 else (10 if h1(14, a) < total(build(4, b)) else total(build(3, b))))
    return (ap(lambda x: x * ap(lambda x: x * 2 + 0, 6) + 2, 5) if ap(lambda x: x * (b if 17 < x else 9) + 3, total(build(4, 12))) < total(build(2, (a - 19))) else b)


def main():
    y = (ap(lambda x: x * pair(11, 2)[1] + 8, ap(lambda x: x * 14 + 8, 14)) ^ 5)
    f = lambda x: x * ap(lambda x: x * total(build(0, 10)) + 6, y) + y
    l = build(6, y)
    return (pair(((y - y) ^ y), y)[1], ap(lambda x: x * (ap(lambda x: x * 18 + 2, 2) if ap(lambda x: x * 0 + 7, y) < ap(lambda x: x * 1 + 0, 12) else total(build(3, 5))) + 4, y), f(pair(((y - y) ^ y), y)[1]) + f(y), total(l) + total(l))
