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
    x = total(build(3, 4))
    return (b if (ap(lambda x: x * 0 + 5, a) ^ ap(lambda x: x * x + 7, b)) < 8 else (pair(a, a)[0] + pair(a, 12)[1]))


def h1(a, b):
    x = pair((total(build(6, 16)) * (15 & a)), 5)[0]
    return total(build(2, (2 + total(build(5, 0)))))


def h2(a, b):
    x = (pair(h1(a, 7), h1(b, a))[0] & ap(lambda x: x * pair(b, b)[1] + 6, 11))
    return 4


def main():
    y = h1(1, pair((3 if 11 < 11 else 3), total(build(2, 6)))[1])
    f = lambda x: x * pair(ap(lambda x: x * 13 + 2, y), 6)[1] + y
    l = build(1, y)
    return (h2(h1(pair(19, 1)[0], (y if 16 < y else y)), y), y, f(h2(h1(pair(19, 1)[0], (y if 16 < y else y)), y)) + f(y), total(l) + total(l))
