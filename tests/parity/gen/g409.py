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
    x = total(build(1, total(build(4, total(build(1, b))))))
    return 10


def h1(a, b):
    x = h0(h0((1 & a), (14 + 15)), pair(b, h0(6, b))[1])
    return total(build(2, pair((x & a), h0(a, x))[1]))


def h2(a, b):
    x = pair(h0(ap(lambda x: x * 2 + 4, 9), h0(b, a)), (5 + 18))[0]
    return ap(lambda x: x * (total(build(4, b)) & total(build(3, 18))) + 0, ap(lambda x: x * (16 * b) + 6, ap(lambda x: x * 2 + 4, b)))


def main():
    y = 12
    f = lambda x: x * total(build(0, 17)) + y
    l = build(6, y)
    return (h0((total(build(5, y)) if 1 < (y + y) else 8), ap(lambda x: x * (y - y) + 5, y)), (h2((10 if y < 10 else 5), ap(lambda x: x * 5 + 3, y)) if 0 < h2(18, total(build(4, 8))) else (total(build(3, 15)) - (y if y < 0 else 2))), f(h0((total(build(5, y)) if 1 < (y + y) else 8), ap(lambda x: x * (y - y) + 5, y))) + f(y), total(l) + total(l))
