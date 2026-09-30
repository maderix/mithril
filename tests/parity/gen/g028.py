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
    x = b
    return total(build(0, total(build(0, (a & 15)))))


def h1(a, b):
    x = total(build(5, 12))
    return total(build(3, (ap(lambda x: x * a + 0, 8) if pair(5, b)[1] < total(build(1, a)) else (a + 3))))


def h2(a, b):
    x = total(build(3, h1(ap(lambda x: x * 1 + 0, 1), b)))
    return ap(lambda x: x * x + 2, 2)


def main():
    y = pair(2, h0(pair(6, 5)[0], total(build(1, 16))))[1]
    f = lambda x: x * pair((y & y), ap(lambda x: x * 15 + 1, y))[1] + y
    l = build(2, y)
    return (y, (h2((y & y), h0(7, y)) if ap(lambda x: x * y + 7, total(build(5, y))) < y else h1(2, (11 if 15 < 13 else y))), f(y) + f(y), total(l) + total(l))
