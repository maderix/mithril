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
    return 1


def h1(a, b):
    x = (pair(14, h0(b, b))[1] if ((a * b) if 9 < h0(a, a) else ap(lambda x: x * 12 + 4, b)) < h0(17, ap(lambda x: x * 1 + 5, b)) else pair(ap(lambda x: x * a + 5, b), h0(a, b))[0])
    return total(build(1, (h0(b, 1) & pair(13, x)[1])))


def h2(a, b):
    x = total(build(4, (h1(5, b) - ap(lambda x: x * a + 3, 3))))
    return total(build(0, b))


def main():
    y = ap(lambda x: x * (7 & pair(7, 5)[0]) + 0, total(build(2, h1(11, 10))))
    f = lambda x: x * (pair(15, y)[0] - 17) + y
    l = build(3, y)
    return (total(build(5, total(build(5, (y & 18))))), pair(ap(lambda x: x * pair(y, 4)[0] + 3, 11), y)[0], f(total(build(5, total(build(5, (y & 18)))))) + f(y), total(l) + total(l))
