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
    x = a
    return b


def h1(a, b):
    x = pair(5, pair((b - 4), total(build(5, 18)))[0])[1]
    return (19 ^ pair(ap(lambda x: x * 7 + 1, 0), ap(lambda x: x * b + 1, 6))[0])


def h2(a, b):
    x = a
    return a


def main():
    y = total(build(6, h0(total(build(1, 9)), (9 ^ 3))))
    f = lambda x: x * (y if (6 if y < y else 1) < pair(y, y)[0] else total(build(4, 16))) + y
    l = build(2, y)
    return (y, ((pair(y, y)[0] if ap(lambda x: x * y + 4, y) < total(build(4, y)) else y) * h0(14, y)), f(y) + f(y), total(l) + total(l))
