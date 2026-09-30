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
    x = 11
    return pair((pair(x, 7)[1] * b), ap(lambda x: x * (6 if b < 8 else 17) + 0, pair(0, b)[1]))[0]


def h1(a, b):
    x = pair(10, b)[0]
    return b


def h2(a, b):
    x = h0(14, (2 + b))
    return ap(lambda x: x * pair(x, (a if 17 < 17 else 16))[1] + 6, 15)


def main():
    y = (15 - h1(h2(18, 16), 10))
    f = lambda x: x * (total(build(6, y)) if total(build(5, 16)) < (y if y < y else y) else 0) + y
    l = build(0, y)
    return ((3 + h0((2 if y < y else y), pair(y, y)[1])), (total(build(3, total(build(2, y)))) + 10), f((3 + h0((2 if y < y else y), pair(y, y)[1]))) + f(y), total(l) + total(l))
