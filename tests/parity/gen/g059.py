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
    x = total(build(3, ap(lambda x: x * ap(lambda x: x * 6 + 5, 14) + 4, (11 ^ 13))))
    return a


def h1(a, b):
    x = ap(lambda x: x * 6 + 0, h0((b & 3), (a - b)))
    return x


def h2(a, b):
    x = total(build(0, h0(total(build(0, 15)), (15 * b))))
    return (pair((3 * x), total(build(0, a)))[0] + pair(total(build(4, 3)), a)[0])


def main():
    y = total(build(2, h0(2, total(build(6, 15)))))
    f = lambda x: x * 1 + y
    l = build(4, y)
    return ((total(build(6, ap(lambda x: x * y + 3, 3))) if ((y ^ y) + h1(y, 12)) < total(build(3, y)) else total(build(4, h2(3, y)))), (1 if h0(h1(3, y), pair(y, 14)[1]) < h2((11 if y < 4 else y), 17) else total(build(6, pair(17, y)[0]))), f((total(build(6, ap(lambda x: x * y + 3, 3))) if ((y ^ y) + h1(y, 12)) < total(build(3, y)) else total(build(4, h2(3, y))))) + f(y), total(l) + total(l))
