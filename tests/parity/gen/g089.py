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
    x = (pair(15, total(build(5, a)))[1] if 15 < pair(total(build(4, 17)), 8)[0] else b)
    return (total(build(3, total(build(2, 7)))) if ap(lambda x: x * b + 7, (18 ^ 8)) < (ap(lambda x: x * x + 8, b) if pair(11, a)[1] < 2 else (b if 18 < x else a)) else total(build(2, ap(lambda x: x * 10 + 4, b))))


def h1(a, b):
    x = a
    return x


def h2(a, b):
    x = (b ^ ap(lambda x: x * ap(lambda x: x * 5 + 3, b) + 8, (b if 4 < 17 else 2)))
    return x


def main():
    y = pair(total(build(3, 6)), ((13 if 8 < 16 else 8) - (0 - 5)))[0]
    f = lambda x: x * ap(lambda x: x * total(build(0, y)) + 0, pair(18, 14)[1]) + y
    l = build(4, y)
    return (total(build(6, y)), h2(10, h1(y, h0(4, 10))), f(total(build(6, y))) + f(y), total(l) + total(l))
