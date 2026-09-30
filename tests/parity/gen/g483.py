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
    x = 16
    return 19


def h1(a, b):
    x = a
    return 15


def h2(a, b):
    x = pair(total(build(3, (b - b))), (10 & h0(12, b)))[0]
    return total(build(0, (h1(16, 5) ^ x)))


def main():
    y = ((ap(lambda x: x * 18 + 8, 0) if 12 < ap(lambda x: x * 13 + 4, 6) else 13) ^ ap(lambda x: x * 19 + 5, total(build(6, 6))))
    f = lambda x: x * ap(lambda x: x * pair(y, y)[1] + 7, (4 - 17)) + y
    l = build(4, y)
    return (total(build(3, total(build(4, 4)))), ap(lambda x: x * (total(build(3, 19)) * h0(9, y)) + 6, total(build(1, pair(4, 1)[1]))), f(total(build(3, total(build(4, 4))))) + f(y), total(l) + total(l))
