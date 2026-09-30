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
    x = ap(lambda x: x * pair(10, (b & 11))[1] + 2, pair(15, 1)[1])
    return ap(lambda x: x * (17 if ap(lambda x: x * a + 2, x) < 9 else ap(lambda x: x * 11 + 4, 14)) + 0, total(build(6, b)))


def h1(a, b):
    x = pair(a, h0((4 if 15 < 18 else 0), total(build(6, a))))[0]
    return h0(((a if 17 < x else b) & (x & a)), (17 if (12 if b < b else b) < (b if 4 < 7 else 19) else (18 - 17)))


def h2(a, b):
    x = 17
    return 14


def main():
    y = 1
    f = lambda x: x * pair(ap(lambda x: x * 7 + 8, 4), y)[1] + y
    l = build(1, y)
    return (h0(pair(12, total(build(0, y)))[0], 14), pair(h1((y & y), y), 11)[1], f(h0(pair(12, total(build(0, y)))[0], 14)) + f(y), total(l) + total(l))
