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
    x = total(build(5, (19 if 17 < 10 else total(build(1, 4)))))
    return a


def h1(a, b):
    x = 3
    return pair((ap(lambda x: x * a + 0, b) if 11 < x else a), pair(2, 16)[0])[0]


def h2(a, b):
    x = ((ap(lambda x: x * b + 6, 16) if (b if 14 < 4 else a) < 10 else ap(lambda x: x * 4 + 8, b)) if (ap(lambda x: x * 14 + 1, 9) if total(build(0, 12)) < total(build(4, 17)) else a) < 8 else total(build(0, ap(lambda x: x * 15 + 1, 15))))
    return x


def main():
    y = h1(((18 if 6 < 0 else 5) * (13 * 16)), 13)
    f = lambda x: x * y + y
    l = build(3, y)
    return (ap(lambda x: x * total(build(4, total(build(6, y)))) + 5, (11 & ap(lambda x: x * y + 1, 17))), y, f(ap(lambda x: x * total(build(4, total(build(6, y)))) + 5, (11 & ap(lambda x: x * y + 1, 17)))) + f(y), total(l) + total(l))
