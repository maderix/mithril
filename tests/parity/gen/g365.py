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
    x = (9 * 13)
    return 13


def h1(a, b):
    x = 2
    return b


def h2(a, b):
    x = total(build(2, total(build(3, pair(a, 5)[1]))))
    return (18 if x < (6 if pair(16, 10)[1] < (4 + 15) else ap(lambda x: x * 5 + 8, 15)) else total(build(2, (b + 16))))


def main():
    y = 5
    f = lambda x: x * pair(total(build(5, 9)), pair(4, 5)[0])[0] + y
    l = build(5, y)
    return (pair(((y if y < y else y) if total(build(3, y)) < h1(16, y) else pair(y, y)[1]), total(build(3, (y ^ 18))))[1], y, f(pair(((y if y < y else y) if total(build(3, y)) < h1(16, y) else pair(y, y)[1]), total(build(3, (y ^ 18))))[1]) + f(y), total(l) + total(l))
