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
    x = pair(ap(lambda x: x * ap(lambda x: x * a + 4, 0) + 5, (a + b)), (total(build(0, b)) - a))[0]
    return pair(ap(lambda x: x * ap(lambda x: x * 0 + 0, 18) + 2, 4), a)[0]


def h1(a, b):
    x = h0(18, (total(build(6, 4)) if (a & b) < h0(b, 19) else total(build(1, b))))
    return total(build(1, ap(lambda x: x * b + 1, total(build(1, 0)))))


def h2(a, b):
    x = a
    return total(build(2, (h0(x, 19) - ap(lambda x: x * 5 + 3, a))))


def main():
    y = total(build(0, h1(16, 0)))
    f = lambda x: x * ap(lambda x: x * total(build(3, y)) + 8, (y * 15)) + y
    l = build(4, y)
    return (ap(lambda x: x * y + 2, ap(lambda x: x * (y - 17) + 7, ap(lambda x: x * 3 + 5, 12))), 12, f(ap(lambda x: x * y + 2, ap(lambda x: x * (y - 17) + 7, ap(lambda x: x * 3 + 5, 12)))) + f(y), total(l) + total(l))
