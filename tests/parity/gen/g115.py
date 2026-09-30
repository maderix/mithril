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
    x = ap(lambda x: x * total(build(3, (b if 12 < b else 12))) + 6, pair(b, pair(12, 5)[0])[0])
    return pair(6, 12)[1]


def h1(a, b):
    x = 1
    return h0(4, a)


def h2(a, b):
    x = h1((h1(14, 14) if 9 < h0(a, 9) else (15 * 10)), pair((0 + 3), ap(lambda x: x * b + 1, a))[1])
    return pair((pair(x, b)[1] & total(build(3, 2))), 7)[0]


def main():
    y = ap(lambda x: x * 14 + 3, ((19 ^ 1) & h0(8, 16)))
    f = lambda x: x * pair(y, total(build(6, 14)))[0] + y
    l = build(2, y)
    return (ap(lambda x: x * (total(build(1, 10)) + h0(y, y)) + 5, (ap(lambda x: x * y + 0, y) - ap(lambda x: x * 14 + 4, y))), ap(lambda x: x * (pair(13, 16)[1] & total(build(6, y))) + 7, 18), f(ap(lambda x: x * (total(build(1, 10)) + h0(y, y)) + 5, (ap(lambda x: x * y + 0, y) - ap(lambda x: x * 14 + 4, y)))) + f(y), total(l) + total(l))
