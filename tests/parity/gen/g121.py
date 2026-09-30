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
    x = total(build(0, ap(lambda x: x * 10 + 0, 4)))
    return ap(lambda x: x * pair(14, x)[1] + 7, ap(lambda x: x * 0 + 7, pair(a, a)[0]))


def h1(a, b):
    x = pair((3 - (12 if 0 < a else b)), ((b if 14 < b else 17) if (b if 0 < 1 else b) < a else total(build(3, 4))))[1]
    return b


def h2(a, b):
    x = a
    return 4


def main():
    y = ap(lambda x: x * ((14 if 13 < 14 else 17) + 16) + 4, total(build(6, (17 - 14))))
    f = lambda x: x * h1(ap(lambda x: x * y + 8, 1), ap(lambda x: x * y + 8, 3)) + y
    l = build(6, y)
    return (((y if ap(lambda x: x * 10 + 6, 1) < y else total(build(6, y))) if total(build(6, (y - y))) < 2 else ap(lambda x: x * 6 + 4, pair(3, 9)[1])), pair(total(build(4, ap(lambda x: x * y + 5, y))), total(build(4, y)))[1], f(((y if ap(lambda x: x * 10 + 6, 1) < y else total(build(6, y))) if total(build(6, (y - y))) < 2 else ap(lambda x: x * 6 + 4, pair(3, 9)[1]))) + f(y), total(l) + total(l))
