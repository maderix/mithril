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
    x = total(build(6, a))
    return b


def h1(a, b):
    x = a
    return 19


def h2(a, b):
    x = (a * 18)
    return 12


def main():
    y = total(build(5, 18))
    f = lambda x: x * h2((y if y < y else 10), pair(y, y)[0]) + y
    l = build(4, y)
    return (h0(ap(lambda x: x * (y if 16 < y else 0) + 0, 18), h2(ap(lambda x: x * y + 0, y), total(build(2, 15)))), (h0(pair(y, y)[1], y) & 8), f(h0(ap(lambda x: x * (y if 16 < y else 0) + 0, 18), h2(ap(lambda x: x * y + 0, y), total(build(2, 15))))) + f(y), total(l) + total(l))
