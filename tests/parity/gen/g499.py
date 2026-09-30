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
    x = total(build(0, 12))
    return x


def h1(a, b):
    x = (h0((a * 10), b) if ap(lambda x: x * (19 + b) + 1, (9 - 4)) < b else a)
    return total(build(6, pair(total(build(5, 12)), b)[1]))


def h2(a, b):
    x = 19
    return ap(lambda x: x * ap(lambda x: x * (b if 6 < b else 19) + 2, ap(lambda x: x * 12 + 6, b)) + 8, total(build(6, (6 if b < b else a))))


def main():
    y = 0
    f = lambda x: x * ap(lambda x: x * (y - y) + 3, (y - y)) + y
    l = build(6, y)
    return (18, (y if 9 < h0(y, 16) else total(build(1, total(build(6, y))))), f(18) + f(y), total(l) + total(l))
