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
    x = ap(lambda x: x * ap(lambda x: x * b + 4, 15) + 2, 10)
    return total(build(1, 8))


def h1(a, b):
    x = ap(lambda x: x * 0 + 8, b)
    return total(build(0, ap(lambda x: x * (x if x < x else b) + 3, (x - x))))


def h2(a, b):
    x = (15 * ((13 if 0 < b else 6) if total(build(5, b)) < (8 if 3 < b else a) else ap(lambda x: x * a + 6, 15)))
    return pair(h1(pair(a, x)[1], (b if b < 4 else x)), pair((a if 0 < a else 1), (a & x))[0])[0]


def main():
    y = 14
    f = lambda x: x * total(build(2, total(build(5, y)))) + y
    l = build(5, y)
    return ((y & h1(pair(8, 0)[1], pair(y, y)[0])), ap(lambda x: x * ((19 if y < y else 0) & ap(lambda x: x * 2 + 6, y)) + 6, ((1 & y) & pair(y, 19)[0])), f((y & h1(pair(8, 0)[1], pair(y, y)[0]))) + f(y), total(l) + total(l))
