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
    x = b
    return (16 if total(build(4, pair(x, 11)[0])) < ((a - b) + ap(lambda x: x * 0 + 4, x)) else ((x - x) * 17))


def h1(a, b):
    x = h0(h0(ap(lambda x: x * b + 3, a), a), (ap(lambda x: x * 16 + 0, 14) ^ total(build(5, 19))))
    return 15


def h2(a, b):
    x = (15 * pair(total(build(6, a)), (b if 4 < 6 else 5))[1])
    return a


def main():
    y = 10
    f = lambda x: x * ap(lambda x: x * pair(y, y)[0] + 4, 9) + y
    l = build(3, y)
    return ((ap(lambda x: x * pair(y, 11)[1] + 7, 18) if total(build(4, 17)) < ap(lambda x: x * (y if y < y else 11) + 0, h2(y, y)) else ap(lambda x: x * y + 4, pair(y, 16)[1])), (y - total(build(1, (y if 17 < y else 10)))), f((ap(lambda x: x * pair(y, 11)[1] + 7, 18) if total(build(4, 17)) < ap(lambda x: x * (y if y < y else 11) + 0, h2(y, y)) else ap(lambda x: x * y + 4, pair(y, 16)[1]))) + f(y), total(l) + total(l))
