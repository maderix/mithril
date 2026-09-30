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
    x = total(build(0, 2))
    return 11


def h1(a, b):
    x = ((14 ^ ap(lambda x: x * b + 5, a)) if total(build(2, 17)) < (a * pair(1, a)[0]) else b)
    return b


def h2(a, b):
    x = pair(total(build(2, total(build(3, b)))), (ap(lambda x: x * 10 + 0, 7) if total(build(0, 18)) < pair(b, a)[1] else ap(lambda x: x * 18 + 8, a)))[0]
    return h1(7, b)


def main():
    y = 11
    f = lambda x: x * pair((18 if 4 < y else 12), total(build(2, y)))[1] + y
    l = build(3, y)
    return (9, ap(lambda x: x * (y + 9) + 8, y), f(9) + f(y), total(l) + total(l))
