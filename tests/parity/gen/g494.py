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
    x = total(build(5, total(build(0, b))))
    return (ap(lambda x: x * pair(a, 2)[0] + 0, total(build(0, b))) & ((12 if b < a else a) if 1 < total(build(5, b)) else total(build(5, 5))))


def h1(a, b):
    x = b
    return 8


def h2(a, b):
    x = total(build(0, a))
    return (7 & b)


def main():
    y = 6
    f = lambda x: x * (ap(lambda x: x * y + 8, y) - y) + y
    l = build(0, y)
    return (11, total(build(5, (y if 8 < 13 else total(build(3, 9))))), f(11) + f(y), total(l) + total(l))
