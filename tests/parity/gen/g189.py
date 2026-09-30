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
    x = total(build(1, pair((a + 8), (b & a))[0]))
    return total(build(6, x))


def h1(a, b):
    x = (total(build(6, total(build(1, 11)))) if 11 < (h0(8, a) if (a if a < 14 else b) < (a if 8 < a else a) else total(build(6, 13))) else ap(lambda x: x * h0(b, 10) + 1, a))
    return ap(lambda x: x * pair((17 ^ 4), (x if 5 < x else 3))[1] + 1, (a + h0(0, 0)))


def h2(a, b):
    x = ((total(build(4, b)) * pair(a, b)[0]) if b < 0 else total(build(1, total(build(5, b)))))
    return (ap(lambda x: x * 7 + 5, ap(lambda x: x * 12 + 2, b)) if ap(lambda x: x * pair(x, b)[0] + 0, b) < (15 + x) else h1(h1(10, 10), (b if a < x else b)))


def main():
    y = h1(ap(lambda x: x * (15 if 2 < 15 else 17) + 3, ap(lambda x: x * 3 + 3, 0)), total(build(4, (9 ^ 0))))
    f = lambda x: x * total(build(0, (y if 1 < y else 4))) + y
    l = build(3, y)
    return ((total(build(0, pair(5, 6)[1])) & ap(lambda x: x * (y & 0) + 0, total(build(2, y)))), total(build(2, h1(total(build(5, y)), ap(lambda x: x * 16 + 4, 1)))), f((total(build(0, pair(5, 6)[1])) & ap(lambda x: x * (y & 0) + 0, total(build(2, y))))) + f(y), total(l) + total(l))
