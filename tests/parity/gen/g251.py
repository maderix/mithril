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
    x = total(build(1, 16))
    return 9


def h1(a, b):
    x = total(build(0, pair(15, ap(lambda x: x * 12 + 0, 15))[1]))
    return x


def h2(a, b):
    x = b
    return a


def main():
    y = (h2((14 if 14 < 2 else 0), total(build(4, 8))) ^ 15)
    f = lambda x: x * h1(pair(y, 9)[1], 11) + y
    l = build(0, y)
    return (ap(lambda x: x * pair((13 if 18 < y else y), 13)[1] + 4, ((y if y < y else y) * pair(16, y)[1])), h0((ap(lambda x: x * 19 + 1, y) if (y * y) < ap(lambda x: x * 13 + 2, 10) else total(build(4, 7))), (y if ap(lambda x: x * 10 + 3, 12) < ap(lambda x: x * y + 4, 13) else h0(y, 13))), f(ap(lambda x: x * pair((13 if 18 < y else y), 13)[1] + 4, ((y if y < y else y) * pair(16, y)[1]))) + f(y), total(l) + total(l))
