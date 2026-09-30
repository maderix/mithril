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
    x = total(build(4, 8))
    return ap(lambda x: x * a + 3, ap(lambda x: x * 1 + 8, a))


def h1(a, b):
    x = a
    return ap(lambda x: x * total(build(3, h0(b, 4))) + 7, 4)


def h2(a, b):
    x = ap(lambda x: x * pair(h0(a, 5), (11 ^ 12))[0] + 0, pair(pair(10, a)[0], total(build(4, a)))[0])
    return h1(17, total(build(0, b)))


def main():
    y = (ap(lambda x: x * h1(15, 7) + 6, (14 - 14)) & total(build(6, total(build(4, 10)))))
    f = lambda x: x * y + y
    l = build(5, y)
    return (y, h1((pair(15, 4)[1] if pair(y, 19)[0] < (y - 6) else h2(y, 8)), 0), f(y) + f(y), total(l) + total(l))
