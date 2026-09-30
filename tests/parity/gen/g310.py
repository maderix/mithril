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
    x = ap(lambda x: x * b + 7, (16 ^ ap(lambda x: x * b + 2, 16)))
    return a


def h1(a, b):
    x = total(build(6, 15))
    return a


def h2(a, b):
    x = h1((9 if h1(5, b) < 3 else h1(14, b)), 5)
    return total(build(2, x))


def main():
    y = 13
    f = lambda x: x * pair(y, (5 & y))[0] + y
    l = build(1, y)
    return (h2(total(build(4, total(build(1, y)))), y), (total(build(6, ap(lambda x: x * 12 + 5, 15))) if total(build(4, (y if y < 17 else y))) < y else (6 if ap(lambda x: x * 8 + 5, y) < ap(lambda x: x * 17 + 1, y) else y)), f(h2(total(build(4, total(build(1, y)))), y)) + f(y), total(l) + total(l))
