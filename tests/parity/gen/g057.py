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
    x = 13
    return total(build(3, a))


def h1(a, b):
    x = ap(lambda x: x * ap(lambda x: x * 12 + 3, pair(10, a)[1]) + 0, h0((19 + 0), ap(lambda x: x * a + 8, 9)))
    return (total(build(3, ap(lambda x: x * b + 2, a))) * (9 if total(build(3, 14)) < h0(9, 16) else ap(lambda x: x * a + 6, b)))


def h2(a, b):
    x = 4
    return h0((x ^ (a if 16 < 13 else a)), x)


def main():
    y = total(build(5, h2(16, (6 if 10 < 11 else 15))))
    f = lambda x: x * total(build(5, h0(y, 18))) + y
    l = build(3, y)
    return ((pair(17, (y if 14 < y else 7))[0] + 11), total(build(3, h0(y, total(build(4, y))))), f((pair(17, (y if 14 < y else 7))[0] + 11)) + f(y), total(l) + total(l))
