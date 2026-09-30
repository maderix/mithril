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
    x = pair(pair(b, ap(lambda x: x * 3 + 7, 1))[0], (ap(lambda x: x * a + 5, a) if (a if a < a else 7) < b else total(build(3, 4))))[0]
    return 18


def h1(a, b):
    x = (total(build(4, 19)) * total(build(2, a)))
    return pair(ap(lambda x: x * h0(12, x) + 2, 8), pair(h0(16, b), ap(lambda x: x * 6 + 6, x))[0])[1]


def h2(a, b):
    x = (ap(lambda x: x * total(build(1, b)) + 0, total(build(5, b))) if (a if h1(a, 6) < (13 if b < b else a) else h0(b, 1)) < ap(lambda x: x * pair(11, 1)[0] + 2, (a if a < 13 else b)) else total(build(3, b)))
    return a


def main():
    y = 3
    f = lambda x: x * (7 if (14 if 13 < 18 else 19) < 14 else (y if y < y else 7)) + y
    l = build(2, y)
    return (0, ap(lambda x: x * 7 + 7, y), f(0) + f(y), total(l) + total(l))
