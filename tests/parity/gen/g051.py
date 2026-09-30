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
    x = pair((total(build(2, 5)) - 11), 4)[1]
    return (a & pair((b if b < 3 else x), 8)[1])


def h1(a, b):
    x = total(build(6, pair((2 if b < 7 else 8), (a if b < 18 else b))[0]))
    return b


def h2(a, b):
    x = total(build(5, h1(17, ap(lambda x: x * b + 0, b))))
    return ((h0(a, b) ^ 8) ^ ap(lambda x: x * pair(6, a)[1] + 0, ap(lambda x: x * b + 3, a)))


def main():
    y = 0
    f = lambda x: x * ap(lambda x: x * 19 + 8, h1(y, y)) + y
    l = build(4, y)
    return (pair(total(build(0, (y if y < 3 else 7))), pair(h1(2, 8), (y if y < 4 else y))[0])[0], pair(total(build(6, ap(lambda x: x * y + 4, 13))), ((6 if 9 < 7 else y) & ap(lambda x: x * y + 4, 8)))[0], f(pair(total(build(0, (y if y < 3 else 7))), pair(h1(2, 8), (y if y < 4 else y))[0])[0]) + f(y), total(l) + total(l))
