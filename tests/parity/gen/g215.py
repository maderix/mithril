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
    x = 2
    return 17


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * ap(lambda x: x * a + 8, 7) + 1, total(build(1, a))) + 6, b)
    return ap(lambda x: x * pair(ap(lambda x: x * a + 7, 18), b)[1] + 5, a)


def h2(a, b):
    x = 10
    return (ap(lambda x: x * total(build(5, 2)) + 5, b) + pair(total(build(1, x)), 9)[0])


def main():
    y = ((11 * (15 + 16)) if (h1(11, 14) if pair(5, 19)[0] < (13 ^ 13) else ap(lambda x: x * 19 + 6, 2)) < total(build(1, ap(lambda x: x * 14 + 6, 14))) else pair(total(build(5, 4)), total(build(5, 5)))[0])
    f = lambda x: x * y + y
    l = build(6, y)
    return (total(build(3, ((y & y) if y < (13 * y) else total(build(5, 19))))), (h2((y if y < 10 else 6), h1(1, y)) - ap(lambda x: x * y + 6, pair(18, 1)[0])), f(total(build(3, ((y & y) if y < (13 * y) else total(build(5, 19)))))) + f(y), total(l) + total(l))
