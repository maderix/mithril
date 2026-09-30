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
    x = pair(a, 15)[1]
    return ap(lambda x: x * pair(total(build(0, x)), pair(15, 0)[1])[0] + 1, (total(build(5, b)) - (b - 4)))


def h1(a, b):
    x = (10 + total(build(4, (b if b < a else a))))
    return b


def h2(a, b):
    x = (h1(h0(b, 14), 6) if 18 < a else 16)
    return h0(h1(total(build(4, b)), total(build(6, 11))), h0(h1(9, x), pair(x, 12)[1]))


def main():
    y = 7
    f = lambda x: x * 3 + y
    l = build(1, y)
    return (pair(ap(lambda x: x * total(build(1, y)) + 7, ap(lambda x: x * 16 + 5, 7)), h1(6, ap(lambda x: x * y + 4, 5)))[0], h0(16, ap(lambda x: x * ap(lambda x: x * y + 5, 12) + 1, pair(4, 3)[0])), f(pair(ap(lambda x: x * total(build(1, y)) + 7, ap(lambda x: x * 16 + 5, 7)), h1(6, ap(lambda x: x * y + 4, 5)))[0]) + f(y), total(l) + total(l))
