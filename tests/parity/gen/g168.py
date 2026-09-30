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
    x = (10 + 13)
    return pair(pair(12, ap(lambda x: x * a + 3, 11))[0], pair(x, ap(lambda x: x * 2 + 6, b))[0])[1]


def h1(a, b):
    x = a
    return (total(build(6, pair(b, 0)[1])) & total(build(4, (0 if a < x else b))))


def h2(a, b):
    x = pair(total(build(2, (8 + a))), ap(lambda x: x * 7 + 5, pair(1, b)[0]))[0]
    return 11


def main():
    y = 18
    f = lambda x: x * (y if 5 < y else h0(7, 18)) + y
    l = build(4, y)
    return (h1(pair(12, (y if y < y else 15))[1], total(build(5, ap(lambda x: x * y + 7, 7)))), 11, f(h1(pair(12, (y if y < y else 15))[1], total(build(5, ap(lambda x: x * y + 7, 7))))) + f(y), total(l) + total(l))
