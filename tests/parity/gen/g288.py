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
    x = total(build(5, total(build(4, pair(14, 13)[0]))))
    return (b if ap(lambda x: x * 0 + 1, ap(lambda x: x * x + 8, x)) < pair(ap(lambda x: x * x + 7, b), x)[1] else (10 if total(build(1, 5)) < (x ^ x) else total(build(0, a))))


def h1(a, b):
    x = (ap(lambda x: x * (13 if 10 < a else 17) + 8, total(build(1, a))) if 18 < total(build(0, total(build(4, 7)))) else h0(14, pair(a, b)[0]))
    return ((total(build(5, x)) if (5 if b < b else 6) < total(build(4, 18)) else pair(14, a)[1]) if 7 < x else pair(h0(x, b), 6)[0])


def h2(a, b):
    x = h0(a, 10)
    return pair((a + (18 if x < x else a)), ap(lambda x: x * (16 * x) + 0, ap(lambda x: x * a + 3, 10)))[0]


def main():
    y = 3
    f = lambda x: x * ap(lambda x: x * ap(lambda x: x * 12 + 3, y) + 8, h1(0, 11)) + y
    l = build(0, y)
    return ((pair(5, 5)[1] if h2(h2(0, 8), y) < total(build(0, ap(lambda x: x * 18 + 0, 18))) else 12), 3, f((pair(5, 5)[1] if h2(h2(0, 8), y) < total(build(0, ap(lambda x: x * 18 + 0, 18))) else 12)) + f(y), total(l) + total(l))
