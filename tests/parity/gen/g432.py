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
    x = pair(19, 5)[1]
    return 15


def h1(a, b):
    x = pair(pair((18 if b < 4 else 19), (11 + b))[1], (a + total(build(4, 7))))[1]
    return ((pair(9, b)[1] + a) if ap(lambda x: x * (a if x < x else 8) + 0, total(build(3, b))) < (pair(x, 19)[1] if ap(lambda x: x * a + 4, a) < total(build(1, 18)) else 10) else a)


def h2(a, b):
    x = total(build(3, total(build(2, (a if 3 < 17 else b)))))
    return ap(lambda x: x * pair((15 if 16 < 7 else 12), (3 * x))[0] + 2, ap(lambda x: x * b + 7, pair(11, a)[0]))


def main():
    y = 6
    f = lambda x: x * h2(total(build(0, 1)), total(build(0, 17))) + y
    l = build(0, y)
    return ((3 ^ total(build(1, pair(y, y)[0]))), h0(h0(ap(lambda x: x * 8 + 4, 15), ap(lambda x: x * 11 + 1, y)), pair(1, pair(17, y)[1])[1]), f((3 ^ total(build(1, pair(y, y)[0])))) + f(y), total(l) + total(l))
