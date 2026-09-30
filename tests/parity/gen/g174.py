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
    x = (ap(lambda x: x * pair(b, 2)[1] + 6, total(build(1, 1))) if total(build(1, pair(10, a)[1])) < 14 else 17)
    return 18


def h1(a, b):
    x = b
    return x


def h2(a, b):
    x = total(build(6, 17))
    return total(build(4, x))


def main():
    y = total(build(5, ap(lambda x: x * total(build(2, 15)) + 2, ap(lambda x: x * 17 + 3, 6))))
    f = lambda x: x * total(build(1, h0(19, 18))) + y
    l = build(0, y)
    return (pair(16, h0((y if y < 0 else 8), 16))[1], ap(lambda x: x * (h0(13, y) + y) + 6, (h1(y, 13) if (15 - y) < h0(y, y) else h0(y, 12))), f(pair(16, h0((y if y < 0 else 8), 16))[1]) + f(y), total(l) + total(l))
